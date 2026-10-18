//! Forem transport: the REST API of one community
//! (`https://<instance>/api/…`, version 1 through its `Accept` header),
//! reads as `GET` with query parameters, writes as `POST` / `PUT` with
//! JSON bodies, the person's API key in the `api-key` header, under ATP's
//! own User-Agent.
//!
//! The instance comes from `FOREM_URL` when set, else from the session
//! (`login --token`), else dev.to; the key from `FOREM_API_KEY` or the
//! session. Forem limits requests per address and answers 429 with
//! `Retry-After`: a short wait is waited out and the request sent again,
//! a long one is `rate_limited`. Errors arrive as `{"error", "status"}`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use atp_core::http::{Method, Req, Resp, TOOL_USER_AGENT, status_error};
use atp_core::{Comment, Ctx, Error, ErrorCode, PageReq, Result, Value, ValueExt, credentials};

pub const DEFAULT_INSTANCE: &str = "https://dev.to";
pub const ENV_INSTANCE: &str = "FOREM_URL";
pub const ENV_KEY: &str = "FOREM_API_KEY";
pub const INSTANCE_KEY: &str = "instance";
pub const KEY_KEY: &str = "api_key";
/// The API version answers follow.
const ACCEPT: &str = "application/vnd.forem.api-v1+json";
/// Tries of one request while Forem asks to wait or fails on its side.
const MAX_TRIES: u32 = 3;
/// The longest `Retry-After` waited out, in seconds.
const MAX_WAIT: u64 = 30;
/// The most rows a listing page may ask for here.
pub const PAGE_MAX: usize = 100;

/// Query parameters.
pub type Query = Vec<(&'static str, String)>;

pub struct Api {
  pub ctx: Ctx,
  /// Comment threads by article id, loaded once for the pages of one run.
  pub threads: RefCell<HashMap<u64, Vec<Comment>>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      threads: RefCell::new(HashMap::new()),
    }
  }

  /// The community's base URL (`https://dev.to`).
  pub fn instance(&self) -> String {
    credentials::env(ENV_INSTANCE)
      .or_else(|| self.ctx.extra(INSTANCE_KEY))
      .filter(|url| !url.is_empty())
      .map(|url| instance_url(&url))
      .unwrap_or_else(|| DEFAULT_INSTANCE.to_owned())
  }

  /// The community's host, for messages (`dev.to`).
  pub fn host(&self) -> String {
    credentials::host(&self.instance())
  }

  pub fn key(&self) -> Option<String> {
    credentials::env(ENV_KEY)
      .or_else(|| self.ctx.extra(KEY_KEY))
      .filter(|key| !key.is_empty())
  }

  pub fn require(&self) -> Result<String> {
    self.key().ok_or_else(|| {
      Error::auth(format!("this needs an API key of {}", self.host()))
        .with_hint("run `atp forem login --token API_KEY` (see `atp guide login forem`)")
    })
  }

  /// `GET /api/<path>`, with the key when there is one.
  pub async fn get(&self, path: &str, query: Query) -> Result<Value> {
    self.send(Method::GET, path, query, None).await
  }

  /// A page of a listing: `page` and `per_page` from the cursor
  /// (`PAGE:SIZE`, so every page keeps the size of the first).
  pub async fn list(
    &self,
    path: &str,
    mut query: Query,
    page: &PageReq,
  ) -> Result<(Vec<Value>, Option<String>)> {
    let (no, size) = page_of(page);
    query.push(("page", no.to_string()));
    query.push(("per_page", size.to_string()));
    let v = self.get(path, query).await?;
    let rows = v.as_array().cloned().unwrap_or_default();
    let next = (rows.len() >= size).then(|| format!("{}:{size}", no + 1));
    Ok((rows, next))
  }

  /// A change with the key, after a short pause; `body` as JSON.
  pub async fn write(&self, method: Method, path: &str, body: Value) -> Result<Value> {
    self.require()?;
    self
      .ctx
      .http
      .pause(Duration::from_millis(500), Duration::from_millis(1500))
      .await;
    self.send(method, path, Vec::new(), Some(body)).await
  }

  async fn send(
    &self,
    method: Method,
    path: &str,
    query: Query,
    body: Option<Value>,
  ) -> Result<Value> {
    let mut tries = 0;
    loop {
      tries += 1;
      let resp = self
        .request(&method, path, &query, body.as_ref())
        .send()
        .await?;
      let status = resp.status.as_u16();
      let wait = retry_after(&resp);
      if status == 429 && tries < MAX_TRIES && wait <= MAX_WAIT {
        tokio::time::sleep(Duration::from_secs(wait.max(1))).await;
        continue;
      }
      if resp.status.is_server_error() && method == Method::GET && tries < MAX_TRIES {
        tokio::time::sleep(Duration::from_secs(u64::from(tries))).await;
        continue;
      }
      return self.check(&resp, wait);
    }
  }

  fn request(&self, method: &Method, path: &str, query: &Query, body: Option<&Value>) -> Req<'_> {
    let mut req = self
      .ctx
      .http
      .request(method.clone(), format!("{}/api/{path}", self.instance()))
      .no_cookies()
      .retries(0)
      .header("user-agent", TOOL_USER_AGENT)
      .header("accept", ACCEPT)
      .queries(query.clone());
    if let Some(key) = self.key() {
      req = req.header("api-key", key);
    }
    match body {
      Some(body) => req.json(body),
      None => req,
    }
  }

  /// The body as JSON, or what went wrong.
  fn check(&self, resp: &Resp, wait: u64) -> Result<Value> {
    let v: Option<Value> = serde_json::from_slice(&resp.body).ok();
    let status = resp.status.as_u16();
    if (200..300).contains(&status) {
      return v
        .ok_or_else(|| Error::upstream(format!("{} did not answer with Forem JSON", self.host())));
    }
    let Some(v) = v else {
      return Err(status_error(resp.status, &resp.text()));
    };
    let message = v.str("error").unwrap_or_else(|| format!("HTTP {status}"));
    let text = format!("{}: {message}", self.host());
    Err(match status {
      401 => Error::auth(text).with_hint("check the API key: `atp guide login forem`"),
      403 => Error::new(ErrorCode::PermissionDenied, text),
      404 => Error::not_found(text),
      429 => Error::new(ErrorCode::RateLimited, text).with_hint(format!(
        "{} asks to wait {wait} s before the next request",
        self.host()
      )),
      400 | 422 => Error::input(text),
      _ => Error::upstream(text),
    })
  }
}

/// `dev.to`, `https://dev.to/` → `https://dev.to`.
pub fn instance_url(raw: &str) -> String {
  let raw = raw.trim().trim_end_matches('/');
  if raw.starts_with("https://") || raw.starts_with("http://") {
    raw.to_owned()
  } else {
    format!("https://{raw}")
  }
}

/// Seconds the `Retry-After` header asks for (a minute when it is missing).
fn retry_after(resp: &Resp) -> u64 {
  resp
    .header("retry-after")
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(60)
}

/// `(page, size)` of a listing (pages count from 1).
fn page_of(page: &PageReq) -> (u64, usize) {
  let parsed = page.cursor.as_deref().and_then(|cursor| {
    let (no, size) = cursor.split_once(':')?;
    Some((no.parse().ok()?, size.parse().ok()?))
  });
  parsed.unwrap_or_else(|| (page.number_or(1), page.size_within(PAGE_MAX)))
}
