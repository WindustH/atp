//! Hupu transport. The forum's pages (`bbs.hupu.com`) are rendered with
//! their data: lists carry it in `window.$$data` (the front page, zones,
//! boards), threads in `__NEXT_DATA__` (the thread, a page of its replies
//! and its 亮了 highlights). ATP reads both from the pages as a browser
//! loads them, without an account.
//!
//! With the site's cookies, the actions go to the JSON API the thread
//! pages call (`bbs.hupu.com/pcmapi/pc/bbs/v1/…`, answering
//! `{code, msg, data}` with `code` 1 for success) and the mentions to the
//! one of the account pages (`my.hupu.com/pcmapi/pc/space/v1/…`).
//!
//! A page Aliyun's WAF stands in front of, a redirect to the login page,
//! HTTP 429 and pages without their data become `verification_required`,
//! `not_authenticated`, `rate_limited` and `upstream_error`; nothing is
//! retried.

use std::time::Duration;

use atp_core::http::{Method, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};

/// The forum; links point here.
pub const BBS: &str = "https://bbs.hupu.com";
/// The account pages (mentions, profiles).
pub const MY: &str = "https://my.hupu.com";
/// The cookie of a login.
pub const LOGIN: &[&str] = &["u"];
/// What marks the page of Aliyun's WAF.
const WAF: &str = "aliyun_waf";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  pub fn logged_in(&self) -> bool {
    LOGIN.iter().all(|c| self.ctx.http.has_cookie(c))
  }

  fn need_login(&self) -> Result<()> {
    if self.logged_in() {
      return Ok(());
    }
    Err(Error::auth("this needs a Hupu login").with_hint(self.ctx.login_hint()))
  }

  /// The HTML of a page of the forum (`path` like `/123.html`).
  pub async fn page(&self, path: &str) -> Result<String> {
    let url = format!("{BBS}{path}");
    let resp = self
      .ctx
      .http
      .get(&url)
      .retries(0)
      .header(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      )
      .header("referer", format!("{BBS}/"))
      .send()
      .await?;
    self.html(resp, &url)
  }

  /// The data a list page was rendered from (`window.$$data`).
  pub async fn list_data(&self, path: &str) -> Result<Value> {
    let html = self.page(path).await?;
    page_data(&html).ok_or_else(|| missing(path))
  }

  /// The props a thread page was rendered from (`__NEXT_DATA__`'s
  /// `props.pageProps`).
  pub async fn thread_data(&self, path: &str) -> Result<Value> {
    let html = self.page(path).await?;
    let data = atp_core::json::next_data(&html).ok_or_else(|| missing(path))?;
    Ok(data.at("props.pageProps").clone())
  }

  fn html(&self, resp: Resp, url: &str) -> Result<String> {
    let status = resp.status.as_u16();
    let text = resp.text();
    if text.get(..16_384).unwrap_or(&text).contains(WAF) {
      return Err(
        Error::verification("Hupu's check (Aliyun WAF) stopped the request").with_url(url),
      );
    }
    if resp.url.contains("passport.hupu.com") {
      return Err(
        Error::auth("Hupu shows its login page for this").with_hint(self.ctx.login_hint()),
      );
    }
    match status {
      200..=299 => Ok(text),
      404 => Err(Error::not_found(format!("Hupu has no page {url}"))),
      429 => Err(rate_limited("HTTP 429, too many requests")),
      _ => Err(status_error(resp.status, &text)),
    }
  }

  /// `POST <BBS><path>` with a JSON body, as a thread page sends it: needs
  /// the login and waits a few seconds first, as a person would between
  /// actions.
  pub async fn post(&self, path: &str, body: &Value, referer: &str) -> Result<Value> {
    self.need_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    let resp = self
      .request(Method::POST, &format!("{BBS}{path}"), BBS, referer)
      .json(body)
      .send()
      .await?;
    self.check(resp, Read::Strict)
  }

  /// `GET <MY><path>?<params>`, as the account pages ask (needs the login).
  pub async fn my(&self, path: &str, params: &[(&str, String)], referer: &str) -> Result<Value> {
    self.need_login()?;
    let resp = self
      .request(Method::GET, &format!("{MY}{path}"), MY, referer)
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .send()
      .await?;
    self.check(resp, Read::Lenient)
  }

  fn request(
    &self,
    method: Method,
    url: &str,
    origin: &str,
    referer: &str,
  ) -> atp_core::http::Req<'_> {
    self
      .ctx
      .http
      .request(method, url)
      .retries(0)
      .header("accept", "application/json, text/plain, */*")
      .header("origin", origin)
      .header("referer", referer)
  }

  /// The JSON answer of the API, or the error it stands for.
  fn check(&self, resp: Resp, read: Read) -> Result<Value> {
    let status = resp.status.as_u16();
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      if status == 401 || resp.url.contains("passport.hupu.com") {
        return Err(self.refusal(401, "登录"));
      }
      if status == 429 {
        return Err(rate_limited("HTTP 429, too many requests"));
      }
      if !resp.status.is_success() {
        return Err(status_error(resp.status, &resp.text()));
      }
      return Err(Error::upstream(format!(
        "Hupu answered with a page instead of data: {}",
        truncate(&one_line(&resp.text()), 160)
      )));
    };
    let code = v.i64("code");
    let ok = match read {
      Read::Strict => code == Some(1),
      Read::Lenient => code.is_none_or(|c| c <= 1),
    };
    if resp.status.is_success() && ok {
      return Ok(v);
    }
    let message = v
      .first_str(&["msg", "message"])
      .unwrap_or_else(|| format!("HTTP {status}, code {}", v.i64("code").unwrap_or(-1)));
    Err(self.refusal(status, &message))
  }

  /// A refusal of the API as a shared error.
  fn refusal(&self, status: u16, message: &str) -> Error {
    let text = format!("Hupu: {message}");
    let has = |words: &[&str]| words.iter().any(|w| message.contains(w));
    if matches!(status, 401 | 403) || has(&["登录", "未登陆"]) {
      return Error::auth(text).with_hint(self.ctx.login_hint());
    }
    if has(&["验证", "滑块"]) {
      return Error::verification(format!("{text}; ATP does not answer the site's check"))
        .with_url(BBS)
        .unclearable();
    }
    if status == 429 || has(&["频繁", "太快", "稍后"]) {
      return rate_limited(message);
    }
    if status == 404 || has(&["不存在", "已删除", "被删除"]) {
      return Error::not_found(text);
    }
    if has(&["禁言", "封禁", "权限", "无法"]) {
      return Error::new(ErrorCode::PermissionDenied, text);
    }
    Error::upstream(text)
  }
}

/// Which answers count as success: actions answer `code` 1 and refuse
/// with 0; lists answer 0 or 1 and refuse with more.
#[derive(Clone, Copy)]
enum Read {
  Strict,
  Lenient,
}

/// The object after `window.$$data=` in a page.
pub fn page_data(html: &str) -> Option<Value> {
  let start = html.find("window.$$data=")? + "window.$$data=".len();
  serde_json::Deserializer::from_str(&html[start..])
    .into_iter::<Value>()
    .next()?
    .ok()
}

fn missing(path: &str) -> Error {
  Error::upstream(format!(
    "Hupu's page {BBS}{path} came without its data; the site may have changed"
  ))
}

fn rate_limited(why: &str) -> Error {
  Error::new(
    ErrorCode::RateLimited,
    format!("Hupu refused the request ({why})"),
  )
  .with_hint("wait a while before retrying, and ask for fewer pages at a time")
}

/// A JSON body field from a cookie the site's scripts read (`ua`,
/// `smidV2`), empty without it, as the thread pages send it.
pub fn cookie_or_empty(api: &Api, name: &str) -> Value {
  json!(api.ctx.http.cookie(name).unwrap_or_default())
}
