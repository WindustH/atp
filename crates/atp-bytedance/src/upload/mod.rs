//! Files for posts, as the upload SDK of ByteDance's web pages
//! (`@byted/uploader`) sends them: with temporary keys the site hands out
//! ([`Keys`]), the upload gateways signed with them (AWS Signature V4,
//! [`crate::sigv4`]) take images through ImageX (`ApplyImageUpload`, TOS,
//! `CommitImageUpload`) and videos through VOD (`ApplyUploadInner`, TOS,
//! `CommitUploadInner`); the bytes themselves go to TOS ([`tos`]). A video
//! that stopped half-way continues on the next run. Each site's page sets
//! the gateways, their region, space and service, and what it adds to the
//! calls ([`Gateway`]): Douyin's creator platform, TikTok Studio.

mod tos;

use std::path::Path;
use std::time::Duration;

use atp_core::http::{Method, status_error};
use atp_core::text::encode_uri_component;
use atp_core::upload::{LocalFile, Parts, Resume};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};
use rand::Rng;

use self::tos::Node;
use crate::sigv4::{self, Request, Scope};

const RESUME_TTL: Duration = Duration::from_secs(2 * 3600);

/// What one site's page sets for its upload SDK.
#[derive(Debug, Clone)]
pub struct Gateway {
  /// The site's name, in errors (`Douyin`).
  pub site: &'static str,
  /// The page's origin, and the referer of the gateway calls.
  pub origin: String,
  pub referer: String,
  /// The addresses of VOD and ImageX (`https://vod.bytedanceapi.com/`).
  pub vod: String,
  pub imagex: String,
  /// The region the calls are signed for.
  pub region: String,
  /// The VOD space videos go to, the ImageX service images go to.
  pub space: String,
  pub image_service: String,
  /// What the SDK adds to every gateway call (`app_id`, `user_id`).
  pub params: Vec<(String, String)>,
  /// What it adds to `Apply` calls besides (`device_platform`).
  pub apply_params: Vec<(String, String)>,
  /// What it adds to a video's `ApplyUploadInner` besides (`business_tag`).
  pub video_params: Vec<(String, String)>,
  /// The `Functions` of a video's commit (`GetMeta`, a first-frame
  /// `Snapshot`).
  pub video_functions: Value,
  /// Whether gateway calls carry the session's cookies: a gateway on the
  /// page's own origin, which the browser sends them to.
  pub cookies: bool,
}

/// Temporary keys of the upload gateways (no `Debug`: they are secrets).
#[derive(Clone)]
pub struct Keys {
  access_key: String,
  secret: String,
  token: String,
  /// Seconds the gateway's clock is ahead, when it is off by over a minute.
  gap: i64,
}

impl Keys {
  /// The keys a site handed out, with the gateway's time as it told it
  /// (`CurrentTime`: RFC 3339, or Unix seconds or milliseconds), which the
  /// SDK signs on.
  pub fn new(access_key: &str, secret: &str, token: &str, current_time: Option<&str>) -> Self {
    let now = jiff::Timestamp::now().as_second();
    let gap = current_time
      .and_then(|t| {
        t.parse::<jiff::Timestamp>()
          .map(|t| t.as_second())
          .ok()
          .or_else(|| t.trim().parse::<i64>().ok())
          .map(|t| if t > 100_000_000_000 { t / 1000 } else { t })
      })
      .map(|t| t - now)
      .filter(|gap| gap.abs() > 60)
      .unwrap_or(0);
    Self {
      access_key: access_key.to_owned(),
      secret: secret.to_owned(),
      token: token.to_owned(),
      gap,
    }
  }
}

/// An image on the site's storage.
#[derive(Debug, Clone)]
pub struct Image {
  pub uri: String,
  pub width: u64,
  pub height: u64,
}

/// A video on VOD: its id, the first frame when the commit made one (the
/// default cover), and `VideoMeta` when the gateway told it (`Width`,
/// `Height`, `Duration` ...).
#[derive(Debug, Clone)]
pub struct Video {
  pub vid: String,
  pub poster: String,
  pub meta: Value,
}

/// The upload gateways of one site with one set of keys, for `user` (the
/// account's uid, as the SDK's `userId`).
pub struct Uploader<'a> {
  ctx: &'a Ctx,
  gateway: Gateway,
  keys: Keys,
  user: String,
}

impl<'a> Uploader<'a> {
  pub fn new(ctx: &'a Ctx, gateway: Gateway, keys: Keys, user: &str) -> Self {
    Self {
      ctx,
      gateway,
      keys,
      user: user.to_owned(),
    }
  }

  /// Upload an image (ImageX).
  pub async fn image(&self, path: &Path) -> Result<Image> {
    let file = LocalFile::open(path).await?;
    let service = ("ServiceId", self.gateway.image_service.as_str());
    let mut query = self.query(&["ApplyImageUpload", "2018-08-01"], service, &[]);
    self.apply(&mut query, &[]);
    let imagex = self.gateway.imagex.clone();
    let result = self.call(&imagex, "imagex", query, None).await?;
    let address = result.at("UploadAddress");
    let node = Node::of(address, address.str("UploadHosts.0"), self.gateway.site)?;
    let parts = Parts::new(self.ctx, &file, &node.store_uri);
    let mut state = Resume::new(node);
    self.send_file(&file, &parts, &mut state).await?;
    let body = json!({ "SessionKey": state.session.session_key }).to_string();
    let query = self.query(&["CommitImageUpload", "2018-08-01"], service, &[]);
    let result = self
      .call(&imagex, "imagex", query, Some((body, "application/json")))
      .await?;
    parts.forget();
    self.image_of(&result, path).await
  }

  /// Upload a video (VOD); a run that stopped continues where it was.
  pub async fn video(&self, path: &Path) -> Result<Video> {
    let file = LocalFile::open(path).await?;
    let target = format!("{}-vod", self.gateway.site.to_ascii_lowercase());
    let parts = Parts::new(self.ctx, &file, &target);
    let mut state = match parts.resume::<Node>(RESUME_TTL) {
      Some(state) => state,
      None => Resume::new(self.apply_video(&file).await?),
    };
    parts.keep(&state);
    if let Err(e) = self.send_file(&file, &parts, &mut state).await {
      return Err(forget_unless_transient(&parts, e));
    }
    let video = self.commit_video(&state.session).await;
    parts.forget();
    video
  }

  async fn apply_video(&self, file: &LocalFile) -> Result<Node> {
    let size = file.size.to_string();
    let space = ("SpaceName", self.gateway.space.as_str());
    let extra = [("FileType", "video"), ("IsInner", "1"), ("FileSize", &size)];
    let mut query = self.query(&["ApplyUploadInner", "2020-11-19"], space, &extra);
    self.apply(&mut query, &self.gateway.video_params);
    let vod = self.gateway.vod.clone();
    let result = self.call(&vod, "vod", query, None).await?;
    let found = result.at("InnerUploadAddress.UploadNodes.0");
    Node::of(found, found.str("UploadHost"), self.gateway.site)
  }

  async fn commit_video(&self, node: &Node) -> Result<Video> {
    let body = json!({
      "SessionKey": node.session_key,
      "Functions": self.gateway.video_functions,
    })
    .to_string();
    let space = ("SpaceName", self.gateway.space.as_str());
    let query = self.query(&["CommitUploadInner", "2020-11-19"], space, &[]);
    let vod = self.gateway.vod.clone();
    let result = self
      .call(&vod, "vod", query, Some((body, "text/plain;charset=UTF-8")))
      .await?;
    let done = result.at("Results.0");
    let vid = done
      .str("Vid")
      .or_else(|| node.vid.clone())
      .ok_or_else(|| {
        Error::upstream(format!(
          "{}'s VOD committed the upload without a video id",
          self.gateway.site
        ))
      })?;
    Ok(Video {
      vid,
      poster: done.str("PosterUri").unwrap_or_default(),
      meta: done.at("VideoMeta").clone(),
    })
  }

  /// `Action` and `Version`, the space or service, `extra`, then what the
  /// site's SDK adds to every call.
  fn query(
    &self,
    action: &[&str; 2],
    space: (&str, &str),
    extra: &[(&str, &str)],
  ) -> Vec<(String, String)> {
    let mut query = pairs(&[("Action", action[0]), ("Version", action[1]), space]);
    query.extend(pairs(extra));
    query.extend(self.gateway.params.iter().cloned());
    query
  }

  /// What an `Apply` call adds: the site's, `more`, and a random `s`.
  fn apply(&self, query: &mut Vec<(String, String)>, more: &[(String, String)]) {
    query.extend(self.gateway.apply_params.iter().cloned());
    query.extend(more.iter().cloned());
    query.push(("s".into(), random_s()));
  }

  /// A signed call to an upload gateway at `base`; its `Result`.
  async fn call(
    &self,
    base: &str,
    service: &str,
    query: Vec<(String, String)>,
    body: Option<(String, &'static str)>,
  ) -> Result<Value> {
    let method = if body.is_some() {
      Method::POST
    } else {
      Method::GET
    };
    let path = url::Url::parse(base)
      .map(|u| u.path().to_owned())
      .unwrap_or_else(|_| "/".into());
    let headers = self.sign(
      &method,
      service,
      &path,
      &query,
      body.as_ref().map(|(b, _)| b.as_str()),
    )?;
    let g = &self.gateway;
    let get = method == Method::GET;
    let mut req = self
      .ctx
      .http
      .request(method, format!("{base}?{}", url_query(&query)))
      .no_throttle()
      .header("referer", &g.referer);
    if !g.cookies {
      req = req.no_cookies();
    }
    // The browser leaves `origin` out of same-origin GETs.
    if !(g.cookies && get) {
      req = req.header("origin", &g.origin);
    }
    for (name, value) in &headers {
      req = req.header(name, value);
    }
    if let Some((body, content_type)) = body {
      req = req.bytes(body.into_bytes(), content_type);
    }
    let resp = req.send().await?;
    let v = resp
      .value()
      .map_err(|_| status_error(resp.status, &resp.text()))?;
    let error = v.at("ResponseMetadata.Error");
    if !error.is_null() {
      let action = query.first().map(|(_, a)| a.as_str()).unwrap_or_default();
      return Err(Error::upstream(format!(
        "{}'s upload gateway refused {action}: {} {}",
        g.site,
        error.str("Code").unwrap_or_default(),
        error.str("Message").unwrap_or_default()
      )));
    }
    Ok(v.at("Result").clone())
  }

  /// The SigV4 headers of a gateway call, as the SDK signs them (on the
  /// gateway's clock), `authorization` last.
  fn sign(
    &self,
    method: &Method,
    service: &str,
    path: &str,
    query: &[(String, String)],
    body: Option<&str>,
  ) -> Result<Vec<(String, String)>> {
    let now = jiff::Timestamp::now().as_second() + self.keys.gap;
    let now = jiff::Timestamp::from_second(now).map_err(|e| Error::internal(e.to_string()))?;
    let date = sigv4::amz_date(now);
    let hash = sigv4::sha256_hex(body.unwrap_or_default().as_bytes());
    let mut headers = vec![
      ("x-amz-date".to_owned(), date.clone()),
      ("x-amz-security-token".to_owned(), self.keys.token.clone()),
    ];
    if body.is_some() {
      headers.push(("x-amz-content-sha256".to_owned(), hash.clone()));
    }
    let request = Request {
      method: method.as_str(),
      path,
      query,
      headers: &headers,
      payload_hash: &hash,
    };
    let keys = sigv4::Keys {
      access_key: &self.keys.access_key,
      secret: &self.keys.secret,
    };
    let scope = Scope {
      region: &self.gateway.region,
      service,
    };
    let authorization = sigv4::authorization(&request, &keys, &scope, &date);
    headers.push(("authorization".to_owned(), authorization));
    Ok(headers)
  }

  /// The committed image: `Results[0]` with `PluginResult[0]`; its size
  /// read from the file when ImageX leaves it out.
  async fn image_of(&self, result: &Value, path: &Path) -> Result<Image> {
    let uri = result
      .first_str(&["PluginResult.0.ImageUri", "Results.0.Uri"])
      .ok_or_else(|| {
        Error::upstream(format!(
          "{}'s ImageX committed the image without a uri",
          self.gateway.site
        ))
      })?;
    let mut width = result.u64("PluginResult.0.ImageWidth").unwrap_or(0);
    let mut height = result.u64("PluginResult.0.ImageHeight").unwrap_or(0);
    if width == 0 || height == 0 {
      let (w, h) = atp_core::file::Image::read(path).await?.size();
      (width, height) = (w as u64, h as u64);
    }
    Ok(Image { uri, width, height })
  }
}

/// Keep the state of an upload only when another run may continue it.
fn forget_unless_transient(parts: &Parts<'_>, e: Error) -> Error {
  if matches!(e.code, ErrorCode::NetworkError | ErrorCode::RateLimited) {
    return e.with_hint("run the command again to continue the upload");
  }
  parts.forget();
  e
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
  list
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect()
}

/// The query as the SDK writes it: in order, `encodeURIComponent`.
fn url_query(query: &[(String, String)]) -> String {
  query
    .iter()
    .map(|(k, v)| format!("{k}={}", encode_uri_component(v)))
    .collect::<Vec<_>>()
    .join("&")
}

/// `Math.random().toString(36).substr(2)`.
fn random_s() -> String {
  const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
  let mut rng = rand::rng();
  (0..11)
    .map(|_| char::from(DIGITS[rng.random_range(0..DIGITS.len())]))
    .collect()
}
