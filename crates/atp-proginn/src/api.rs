//! Proginn transport: the API of job.proginn.com, the job board, which its
//! own pages call (`/api/recruit/…`, `/api/user/getInfo`,
//! `/api/message/getUnreadCount`). Bodies are forms (`a=1&b=2`), as the
//! site's request library sends them.
//!
//! Answers are `{status, info, data}`: `status` 1 is success and -99 a
//! call that needs a login; anything else comes with the site's words in
//! `info`. Reads need no account. A login is the `x_access_token` cookie
//! www.proginn.com sets on `.proginn.com`, which the board's pages also
//! read to tell whether someone is logged in.

use std::time::Duration;

use atp_core::http::{Method, Req, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result, Value, ValueExt};

/// The job board: its API and the job pages.
pub const JOB: &str = "https://job.proginn.com";
/// The main site, where accounts log in.
pub const WWW: &str = "https://www.proginn.com";
/// The login cookie.
pub const TOKEN: &str = "x_access_token";
/// `status` of a call that needs a login.
pub const NEEDS_LOGIN: i64 = -99;

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(&[TOKEN])
  }

  /// `GET /api/<path>`; the answer's `data`.
  pub async fn get(&self, path: &str) -> Result<Value> {
    let resp = self.request(Method::GET, path).send().await?;
    self.data(json(resp)?)
  }

  /// `POST /api/<path>` with a form body; the answer's `data`.
  pub async fn post(&self, path: &str, form: &[(&str, String)]) -> Result<Value> {
    let v = self.answer(path, form).await?;
    self.data(v)
  }

  /// `POST /api/<path>` with a form body; the whole answer, whatever its
  /// `status`.
  pub async fn answer(&self, path: &str, form: &[(&str, String)]) -> Result<Value> {
    let resp = self
      .request(Method::POST, path)
      .header("origin", JOB)
      .form(form.iter().map(|(k, v)| (*k, v.as_str())))
      .send()
      .await?;
    json(resp)
  }

  /// A read of your own: needs the login.
  pub async fn own(&self, path: &str) -> Result<Value> {
    self.require_login()?;
    self.get(path).await
  }

  /// A change to the account: needs the login and waits a few seconds
  /// first, as a person would between actions.
  pub async fn write(&self, path: &str, form: &[(&str, String)]) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    self.post(path, form).await
  }

  fn request(&self, method: Method, path: &str) -> Req<'_> {
    self
      .ctx
      .http
      .request(method, format!("{JOB}/api/{path}"))
      .header("accept", "application/json, text/plain, */*")
      .header("referer", format!("{JOB}/"))
  }

  /// The `data` of a `{status, info, data}` answer, or the error it
  /// stands for.
  pub fn data(&self, v: Value) -> Result<Value> {
    match v.i64("status") {
      Some(1) => Ok(v.at("data").clone()),
      Some(NEEDS_LOGIN) => Err(self.logged_out(&v)),
      _ => Err(Error::upstream(format!("Proginn: {}", message(&v)))),
    }
  }

  /// A call refused for want of a login.
  fn logged_out(&self, v: &Value) -> Error {
    let text = format!("Proginn: {}", message(v));
    let text = if self.ctx.http.has_cookie(TOKEN) {
      format!("{text} (the session ended)")
    } else {
      format!("{text} (this needs a login)")
    };
    Error::auth(text).with_hint(self.ctx.login_hint())
  }
}

/// The JSON of an answer.
fn json(resp: Resp) -> Result<Value> {
  if !resp.status.is_success() {
    return Err(status_error(resp.status, &resp.text()));
  }
  serde_json::from_slice::<Value>(&resp.body).map_err(|_| {
    Error::upstream(format!(
      "Proginn answered with a page instead of data: {}",
      truncate(&one_line(&resp.text()), 160)
    ))
  })
}

/// The site's words for an answer (`info`), else its status.
pub fn message(v: &Value) -> String {
  v.first_str(&["info", "error"])
    .unwrap_or_else(|| format!("status {}", v.str("status").unwrap_or_default()))
}
