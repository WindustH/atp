//! Eleduck transport: the JSON API eleduck.com's own pages call
//! (`https://svc.eleduck.com/api/v1/…`). Reads need no account. A login is
//! the site's `token` cookie, sent as `Authorization: Bearer <token>` as
//! the web app sends it, with the `X-Fingerprint-Id` the web app adds when
//! the browser's `fingerprint_id` cookie came along with the session.
//!
//! Errors come as `{error, message}` with the HTTP status: 401
//! (`access_not_authorized`) is `not_authenticated`, 404 `not_found`, 403
//! `permission_denied`, 429 `rate_limited`. Listings page by page number
//! (`pager.next_page`), which ATP hands back as its cursor.

use std::time::Duration;

use atp_core::http::{Method, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt};

/// The API every call goes to.
pub const API: &str = "https://svc.eleduck.com/api/v1";
/// The site; links point here.
pub const WEB: &str = "https://eleduck.com";
/// The cookie holding the login token.
pub const TOKEN: &str = "token";
/// The browser's fingerprint cookie, sent back as `X-Fingerprint-Id`.
const FINGERPRINT: &str = "fingerprint_id";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  pub fn token(&self) -> Option<String> {
    self.ctx.http.cookie(TOKEN)
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(&[TOKEN])
  }

  /// `GET <API><path>?<params>`.
  pub async fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    self.call(Method::GET, path, params, None).await
  }

  /// A call that needs the login: `GET` of your own lists.
  pub async fn get_own(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    self.require_login()?;
    self.get(path, params).await
  }

  /// A change to the account: needs the login and waits a few seconds
  /// first, as a person would between actions.
  pub async fn write(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    self.call(method, path, &[], body.as_ref()).await
  }

  /// A call without the login, for the WeChat QR login.
  pub async fn anonymous(
    &self,
    method: Method,
    path: &str,
    params: &[(&str, String)],
  ) -> Result<Resp> {
    self
      .ctx
      .http
      .request(method, format!("{API}{path}"))
      .no_cookies()
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json")
      .header("origin", WEB)
      .header("referer", format!("{WEB}/"))
      .send()
      .await
  }

  async fn call(
    &self,
    method: Method,
    path: &str,
    params: &[(&str, String)],
    body: Option<&Value>,
  ) -> Result<Value> {
    let http = &self.ctx.http;
    let mut req = http
      .request(method, format!("{API}{path}"))
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json")
      .header("origin", WEB)
      .header("referer", format!("{WEB}/"));
    if let Some(token) = self.token() {
      req = req.header("authorization", format!("Bearer {token}"));
    }
    if let Some(fingerprint) = http.cookie(FINGERPRINT) {
      req = req.header("x-fingerprint-id", fingerprint);
    }
    if let Some(body) = body {
      req = req.json(body);
    }
    let resp = req.send().await?;
    self.check(resp)
  }

  /// The JSON answer (`null` for an empty one), or the error it stands for.
  fn check(&self, resp: Resp) -> Result<Value> {
    let status = resp.status.as_u16();
    if resp.body.iter().all(u8::is_ascii_whitespace) && resp.status.is_success() {
      return Ok(Value::Null);
    }
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      if !resp.status.is_success() {
        return Err(status_error(resp.status, &resp.text()));
      }
      return Err(Error::upstream(format!(
        "Eleduck answered with a page instead of data: {}",
        truncate(&one_line(&resp.text()), 160)
      )));
    };
    if resp.status.is_success() {
      return Ok(v);
    }
    Err(self.error(status, &v))
  }

  /// Eleduck's `{error, message}` as a shared error.
  fn error(&self, status: u16, v: &Value) -> Error {
    let message = v
      .first_str(&["message", "error"])
      .unwrap_or_else(|| format!("HTTP {status}"));
    let text = format!("Eleduck: {message}");
    match status {
      401 => {
        let what = if self.token().is_some() {
          format!("{text} (the session ended)")
        } else {
          format!("{text} (this needs a login)")
        };
        Error::auth(what).with_hint(self.ctx.login_hint())
      }
      403 => Error::new(ErrorCode::PermissionDenied, text),
      404 => Error::not_found(text),
      429 => Error::new(ErrorCode::RateLimited, text).with_hint("wait a while before retrying"),
      400 | 422 => Error::input(text),
      _ => Error::upstream(text),
    }
  }
}
