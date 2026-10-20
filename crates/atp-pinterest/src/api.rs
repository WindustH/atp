//! Pinterest transport: the resource API the site's pages call
//! (`www.pinterest.com/resource/<Name>Resource/<action>/`). Reads are
//! `GET …/get/` with the page the request belongs to (`source_url`) and
//! the call's options (`data`: `{"options": {…}, "context": {}}`) as
//! parameters; actions are `POST` with the same fields as a form and the
//! `csrftoken` cookie in `X-CSRFToken`. Every request names the page's
//! handler (`X-Pinterest-PWS-Handler`), as the pages do.
//!
//! Answers are `{resource_response: {status, data, bookmark, error}}`;
//! listings go on with their `bookmark` until it reads `-end-`. Refusals
//! carry their own HTTP status: 401 (and 403 for guests) is
//! `not_authenticated`, 404 `not_found`, 429 `rate_limited`. Reads are
//! tried twice more on network errors and 5xx, actions never.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use atp_core::http::{Method, Req, Resp};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};

/// The site; links point here.
pub const SITE: &str = "https://www.pinterest.com";
/// The cookie of a session (guests get one too; `_auth` tells a login).
pub const LOGIN: &[&str] = &["_pinterest_sess"];
/// The end of a listing, as a bookmark.
const END: &str = "-end-";

/// The data of an answer, and where a listing goes on.
pub struct Answer {
  pub data: Value,
  pub bookmark: Option<String>,
}

impl Answer {
  /// The rows of a listing: `data` itself, or its `results`.
  pub fn rows(&self) -> &[Value] {
    match &self.data {
      Value::Array(rows) => rows,
      other => other.list("results"),
    }
  }
}

pub struct Api {
  pub ctx: Ctx,
  /// Boards by the argument naming them, for the pages of one listing.
  pub boards: RefCell<HashMap<String, Value>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      boards: RefCell::new(HashMap::new()),
    }
  }

  /// Whether the session is logged in: the site sets `_auth=1` then.
  pub fn logged_in(&self) -> bool {
    self.ctx.http.cookie("_auth").as_deref() == Some("1")
  }

  pub fn need_login(&self) -> Result<()> {
    if self.logged_in() {
      return Ok(());
    }
    Err(Error::auth("this needs a Pinterest login").with_hint(self.ctx.login_hint()))
  }

  /// `GET /resource/<resource>/get/` for the page `source` (`/pin/1/`).
  pub async fn get(&self, resource: &str, options: Value, source: &str) -> Result<Answer> {
    let data = json!({ "options": options, "context": {} }).to_string();
    let resp = self
      .request(Method::GET, &format!("/resource/{resource}/get/"), source)
      .query("source_url", source)
      .query("data", data)
      .send()
      .await?;
    self.check(&resp)
  }

  /// `POST /resource/<resource>/<action>/` (`create`, `update`, `delete`):
  /// needs the login, and waits a moment first.
  pub async fn act(
    &self,
    resource: &str,
    action: &str,
    options: Value,
    source: &str,
  ) -> Result<Value> {
    self.need_login()?;
    let csrf = self.ctx.http.cookie("csrftoken").ok_or_else(|| {
      Error::auth("the session has no `csrftoken` cookie for actions")
        .with_hint(self.ctx.login_hint())
    })?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(1), Duration::from_secs(3))
      .await;
    let data = json!({ "options": options, "context": {} }).to_string();
    let form = [("source_url", source.to_owned()), ("data", data)];
    let resp = self
      .request(
        Method::POST,
        &format!("/resource/{resource}/{action}/"),
        source,
      )
      .header("origin", SITE)
      .header("x-csrftoken", csrf)
      .form(form.iter().map(|(k, v)| (*k, v)))
      .send()
      .await?;
    Ok(self.check(&resp)?.data)
  }

  fn request(&self, method: Method, path: &str, source: &str) -> Req<'_> {
    let retries = if method == Method::GET { 2 } else { 0 };
    self
      .ctx
      .http
      .request(method, format!("{SITE}{path}"))
      .retries(retries)
      .header("accept", "application/json, text/javascript, */*, q=0.01")
      .header("x-requested-with", "XMLHttpRequest")
      .header("x-pinterest-appstate", "active")
      .header("x-pinterest-pws-handler", handler(source))
      .header("x-pinterest-source-url", source)
      .header("referer", format!("{SITE}{source}"))
  }

  /// The answer's data, or the error it stands for.
  fn check(&self, resp: &Resp) -> Result<Answer> {
    let status = resp.status.as_u16();
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      if matches!(status, 401 | 403) {
        return Err(self.refusal(status, "the site asks to log in"));
      }
      return Err(Error::upstream(format!(
        "Pinterest answered with a page instead of data (HTTP {status}): {}",
        truncate(&one_line(&resp.text()), 160)
      )));
    };
    let answer = v.at("resource_response");
    let failed = answer.str("status").is_some_and(|s| s != "success");
    if resp.status.is_success() && !failed && !answer.is_null() {
      let bookmark = answer.str("bookmark").filter(|b| b != END);
      return Ok(Answer {
        data: answer.at("data").clone(),
        bookmark,
      });
    }
    let message = answer
      .first_str(&["error.message", "message", "error.code"])
      .unwrap_or_else(|| format!("HTTP {status}"));
    let code = answer
      .u64("error.http_status")
      .map(|s| s as u16)
      .filter(|s| *s >= 400)
      .unwrap_or(status);
    Err(self.refusal(code, &message))
  }

  /// A refusal as a shared error.
  fn refusal(&self, status: u16, message: &str) -> Error {
    let text = format!("Pinterest: {message}");
    match status {
      401 => Error::auth(text).with_hint(self.ctx.login_hint()),
      403 if !self.logged_in() => Error::auth(format!(
        "{text} (Pinterest shows this to logged-in accounts only)"
      ))
      .with_hint(self.ctx.login_hint()),
      403 => Error::new(ErrorCode::PermissionDenied, text),
      404 => Error::not_found(text),
      429 => Error::new(ErrorCode::RateLimited, text)
        .with_hint("wait a while before retrying, and ask for fewer pages at a time"),
      400 => Error::input(text),
      _ => Error::upstream(text),
    }
  }
}

/// The handler of the page a request belongs to, as the site's pages
/// name it.
fn handler(source: &str) -> &'static str {
  let path = source.split('?').next().unwrap_or(source);
  let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
  match parts.as_slice() {
    [] => "www/index.js",
    ["search", ..] => "www/search/[scope].js",
    ["pin", ..] => "www/pin/[id].js",
    [_] => "www/[username].js",
    _ => "www/[username]/[slug].js",
  }
}
