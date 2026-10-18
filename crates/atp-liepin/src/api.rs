//! Liepin transport: the `api-c.liepin.com` gateway the site's pages call
//! (`POST /api/com.liepin.<service>.<method>`), the pages of
//! `www.liepin.com` that jobs and companies are rendered into, the
//! `api-dok` base data (city codes) and the `api-passport` QR login.
//!
//! Every gateway call carries what the site's request library adds: the
//! `X-Client-Type`, `X-Fscp-*` and `X-Requested-With` headers, a fresh
//! `X-Fscp-Trace-Id` and the `XSRF-TOKEN` cookie echoed as
//! `X-XSRF-TOKEN` (double-submit; any `www` page sets the cookie, so one
//! is loaded when it is missing). Bodies are forms (`a=1&b=2`), or JSON
//! wrapped as `{"data": …}` for the search.
//!
//! Answers are `{flag, data}`; `flag` 0 comes with a `code` and a `msg`:
//! `-1401` and `5002` mean not logged in, `-1701` a login replaced by
//! another one. A `TD-SecIntercept-Redirect` header sends the browser to
//! the site's security check (`verification_required`, opened in the
//! browser), and the site's firewall answers bursts with HTTP 405.

use std::time::Duration;

use atp_core::http::{Method, Req, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};

/// The site; job and company pages live here.
pub const WWW: &str = "https://www.liepin.com";
/// The gateway of the job seeker's side.
const API: &str = "https://api-c.liepin.com";
/// Base data: city codes.
const DOK: &str = "https://api-dok.liepin.com";
/// Logins.
const PASSPORT: &str = "https://api-passport.liepin.com";
/// The client id the pages send in `X-Fscp-Std-Info`.
const CLIENT_ID: &str = "40108";
/// The double-submit cookie.
const XSRF: &str = "XSRF-TOKEN";
/// The cookie holding the logged-in account's id.
pub const USER: &str = "UniqueKey";
/// The header that sends the browser to a security check.
const INTERCEPT: &str = "td-secintercept-redirect";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(&[USER])
  }

  /// A cookie of the session (`imId_0` …).
  pub fn cookie(&self, name: &str) -> Option<String> {
    self.ctx.http.cookie(name).filter(|v| !v.is_empty())
  }

  /// The `XSRF-TOKEN` cookie, from a `www` page when the session has none.
  async fn xsrf(&self) -> Result<String> {
    if let Some(token) = self.cookie(XSRF) {
      return Ok(token);
    }
    self.page(&format!("{WWW}/zhaopin/")).await?;
    self
      .cookie(XSRF)
      .ok_or_else(|| Error::upstream("Liepin set no XSRF-TOKEN cookie"))
  }

  /// The headers of the site's request library.
  async fn gateway(&self, url: String, referer: &str) -> Result<Req<'_>> {
    let xsrf = self.xsrf().await?;
    Ok(
      self
        .ctx
        .http
        .request(Method::POST, url)
        .header("accept", "application/json, text/plain, */*")
        .header("origin", WWW)
        .header("referer", referer)
        .header("x-client-type", "web")
        .header("x-requested-with", "XMLHttpRequest")
        .header("x-fscp-version", "1.1")
        .header(
          "x-fscp-std-info",
          format!(r#"{{"client_id": "{CLIENT_ID}"}}"#),
        )
        .header("x-fscp-bi-stat", json!({ "location": referer }).to_string())
        .header("x-fscp-trace-id", uuid())
        .header("x-xsrf-token", xsrf),
    )
  }

  /// `POST /api/com.liepin.<name>` with a JSON body, wrapped in `data`
  /// as the site sends it; the answer's `data`.
  pub async fn json(&self, name: &str, data: Value) -> Result<Value> {
    let referer = format!("{WWW}/zhaopin/");
    let resp = self
      .gateway(format!("{API}/api/com.liepin.{name}"), &referer)
      .await?
      .json(&json!({ "data": data }))
      .send()
      .await?;
    self.check(resp)
  }

  /// `POST /api/com.liepin.<name>` with a form body; the answer's `data`.
  pub async fn form(&self, name: &str, form: &[(&str, String)]) -> Result<Value> {
    self.form_from(name, form, &format!("{WWW}/")).await
  }

  /// A form call as sent from the page at `referer` (the job an
  /// application is for).
  async fn form_from(&self, name: &str, form: &[(&str, String)], referer: &str) -> Result<Value> {
    let url = format!("{API}/api/com.liepin.{name}");
    self.post_form(url, form, referer).await
  }

  async fn post_form(&self, url: String, form: &[(&str, String)], referer: &str) -> Result<Value> {
    let resp = self
      .gateway(url, referer)
      .await?
      .form(form.iter().map(|(k, v)| (*k, v.as_str())))
      .send()
      .await?;
    self.check(resp)
  }

  /// A form call on your own data: needs the login.
  pub async fn own(&self, name: &str, form: &[(&str, String)]) -> Result<Value> {
    self.require_login()?;
    self.form(name, form).await
  }

  /// A change to the account: needs the login and waits a few seconds
  /// first, as a person would between actions.
  pub async fn write(&self, name: &str, form: &[(&str, String)], referer: &str) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    self.form_from(name, form, referer).await
  }

  /// A form call to the base-data service (`api-dok`).
  pub async fn dok(&self, name: &str, form: &[(&str, String)]) -> Result<Value> {
    let url = format!("{DOK}/api/com.liepin.{name}");
    self.post_form(url, form, &format!("{WWW}/zhaopin/")).await
  }

  /// A form call to the login service, which the site sends with
  /// `X-Requested-With` only; the whole answer, `flag` included.
  pub async fn passport(&self, name: &str, form: &[(&str, String)]) -> Result<Value> {
    let resp = self
      .ctx
      .http
      .post(format!("{PASSPORT}/api/com.liepin.{name}"))
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WWW)
      .header("referer", format!("{WWW}/"))
      .header("x-requested-with", "XMLHttpRequest")
      .form(form.iter().map(|(k, v)| (*k, v.as_str())))
      .send()
      .await?;
    self.intercepted(&resp)?;
    resp.check()?.value()
  }

  /// A page of `www.liepin.com` as HTML.
  pub async fn page(&self, url: &str) -> Result<String> {
    let resp = self
      .ctx
      .http
      .get(url)
      .header(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      )
      .header("referer", format!("{WWW}/"))
      .send()
      .await?;
    self.intercepted(&resp)?;
    match resp.status.as_u16() {
      404 => Err(Error::not_found(format!("Liepin has no page {url}"))),
      405 => Err(too_fast()),
      _ => Ok(resp.check()?.text()),
    }
  }

  /// The security check a response sends the browser to.
  fn intercepted(&self, resp: &Resp) -> Result<()> {
    match resp.header(INTERCEPT).filter(|url| !url.is_empty()) {
      Some(url) => Err(
        Error::verification("Liepin asks for a security check (安全验证) before going on")
          .with_url(url),
      ),
      None => Ok(()),
    }
  }

  /// The `data` of a `{flag, data}` answer, or the error it stands for.
  fn check(&self, resp: Resp) -> Result<Value> {
    self.intercepted(&resp)?;
    if resp.status.as_u16() == 405 {
      return Err(too_fast());
    }
    if !resp.status.is_success() {
      return Err(status_error(resp.status, &resp.text()));
    }
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      return Err(Error::upstream(format!(
        "Liepin answered with a page instead of data: {}",
        truncate(&one_line(&resp.text()), 160)
      )));
    };
    if v.i64("flag") == Some(1) {
      return Ok(v.at("data").clone());
    }
    Err(self.error(&v))
  }

  /// Liepin's `{flag: 0, code, msg}` as a shared error.
  fn error(&self, v: &Value) -> Error {
    let code = v.str("code").unwrap_or_default();
    let message = v.str("msg").unwrap_or_else(|| format!("code {code}"));
    let text = format!("Liepin: {message}");
    match code.as_str() {
      "-1401" | "5002" | "-1701" => {
        let text = if self.cookie(USER).is_some() {
          format!("{text} (the session ended)")
        } else {
          format!("{text} (this needs a login)")
        };
        Error::auth(text).with_hint(self.ctx.login_hint())
      }
      "-1400" => Error::input(format!("{text} (the request was refused as malformed)")),
      _ => Error::upstream(format!("{text} ({code})")),
    }
  }
}

/// What HTTP 405 from the site's firewall means.
fn too_fast() -> Error {
  Error::new(
    ErrorCode::RateLimited,
    "Liepin refused the request (HTTP 405), as its firewall does with requests that come too fast",
  )
  .with_hint("wait a few minutes before retrying")
}

/// A random version-4 UUID, as `X-Fscp-Trace-Id`.
fn uuid() -> String {
  let mut b: [u8; 16] = rand::random();
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
  format!(
    "{}-{}-{}-{}-{}",
    &hex[..8],
    &hex[8..12],
    &hex[12..16],
    &hex[16..20],
    &hex[20..]
  )
}

/// 32 random hex digits: the `ckId` the search page makes for each search.
pub fn search_id() -> String {
  let b: [u8; 16] = rand::random();
  b.iter().map(|x| format!("{x:02x}")).collect()
}
