//! Calls TikTok Studio's own page makes, in ATP's browser tab
//! ([`Platform::BROWSER_TAB`](atp_core::Platform::BROWSER_TAB)). Studio's
//! pages sign some paths beyond the `X-Bogus` ATP computes: their
//! `byted_acrawler` adds `_signature` to the upload keys
//! (`/api/v1/video/upload/auth/`), publishing (`/tiktok/web/project/post/v1/`)
//! and editing (`/tiktok/post/edit/v1/`), their ZTI SDK adds ticket-guard
//! headers to the last two, and `webmssdk` adds `X-Bogus`, `X-Gnarly` and
//! `msToken`. ATP computes none of that: those calls go out as the page's
//! own `fetch`, from a Studio page loaded in the tab with the session's
//! cookies, and the page's scripts add what they add. ATP waits until the
//! page has signed a request of its own (its scripts start with its first
//! calls), adds only what the page's request layer does (`aid`, the device
//! id and account as axios's defaults, the `tt-csrf-token` header) and
//! reads the answer.

use atp_core::tab::Tab;
use atp_core::{Error, Result, Value, ValueExt, json};

use crate::api::{Api, WWW, encode, status_error};

/// Studio's upload page, as its menu opens it.
pub const UPLOAD_PAGE: &str = "https://www.tiktok.com/tiktokstudio/upload?from=creator_center";
/// Studio's list of posts, whose scripts sign edits too.
pub const CONTENT_PAGE: &str = "https://www.tiktok.com/tiktokstudio/content";
const AID: &str = "1988";

/// A Studio page in the browser tab.
pub struct Browser<'a> {
  pub api: &'a Api,
  tab: Tab<'a>,
  page: &'static str,
}

impl<'a> Browser<'a> {
  /// Open `page` in the tab, logged in, and wait until it has signed a
  /// request of its own: its signing scripts start with the page's first
  /// calls, and sign what is asked from the page once they run.
  pub async fn open(api: &'a Api, page: &'static str) -> Result<Self> {
    api.require_login()?;
    let tab = api.ctx.tab().await?;
    crate::tab::ready(&tab, page).await?;
    let at = tab.eval("location.href").await?;
    let at = at.as_str().unwrap_or_default();
    if !at.contains("/tiktokstudio") {
      return Err(
        Error::auth(format!(
          "TikTok Studio did not open; the browser shows {at}"
        ))
        .with_hint(api.ctx.login_hint()),
      );
    }
    Ok(Self { api, tab, page })
  }

  /// A GET the page makes.
  pub async fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    check(path, self.call("GET", path, params, None).await?)
  }

  /// A POST with a JSON body the page makes; its answer as it is, which
  /// may carry an error code ([`check`]).
  pub async fn post(&self, path: &str, params: &[(&str, String)], body: &Value) -> Result<Value> {
    self.call("POST", path, params, Some(body)).await
  }

  /// A JavaScript expression evaluated in the page.
  pub async fn eval(&self, expression: &str) -> Result<Value> {
    self.tab.eval(expression).await
  }

  async fn call(
    &self,
    method: &str,
    path: &str,
    params: &[(&str, String)],
    body: Option<&Value>,
  ) -> Result<Value> {
    let url = format!("{WWW}{path}?{}", self.query(params).await?);
    let mut headers = json!({ "accept": "application/json, text/plain, */*" });
    if let Some(token) = self.api.ctx.http.cookie("tt_csrf_token") {
      headers["tt-csrf-token"] = token.into();
    }
    let mut init = json!({ "method": method });
    if let Some(body) = body {
      headers["content-type"] = "application/json".into();
      init["body"] = body.to_string().into();
    }
    init["headers"] = headers;
    let answer = self.tab.fetch(self.page, &url, init).await?;
    // What the page's scripts made of the address (`X-Bogus`, `_signature` ...).
    tracing::debug!("{path} went out as {}", answer.url);
    answer.json()
  }

  /// `params`, then what the page's request layer adds to every call.
  async fn query(&self, params: &[(&str, String)]) -> Result<String> {
    let app = self.api.app().await?;
    let uid = app
      .user
      .as_ref()
      .and_then(|u| u.str("uid"))
      .unwrap_or_default();
    let mut all: Vec<(&str, String)> = params.to_vec();
    all.push(("aid", AID.into()));
    all.push(("device_id", app.web_id.clone()));
    all.push(("odinId", uid));
    Ok(
      all
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&"),
    )
  }
}

/// `status_code` of an answer, mapped to errors.
pub fn check(path: &str, v: Value) -> Result<Value> {
  let code = v
    .i64("status_code")
    .or_else(|| v.i64("statusCode"))
    .unwrap_or(0);
  if code == 0 {
    return Ok(v);
  }
  let msg = v
    .first_str(&["status_msg", "statusMsg", "message"])
    .unwrap_or_default();
  Err(status_error(path, code, &msg))
}
