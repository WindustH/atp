//! Shixiseng transport. The site's pages talk over two hosts:
//!
//! - `www.shixiseng.com`: the job and company search of its search page
//!   (`/app/interns/search/v2`, `/company/search`, [`Route::Site`]), the
//!   font of the search's cards, and `/proxy-prefix/<service>/api/…`, the
//!   site's server passing calls on to its API gateway, one prefix per
//!   service ([`Route::Interns`]: job and company pages, [`Route::Account`]:
//!   the account and saved jobs, [`Route::Deliver`]: applications);
//! - `apigateway.shixiseng.com`: the login panel's WeChat mini-program
//!   code ([`Api::gateway`]).
//!
//! Every answer is `{code, msg}`, `code` 100 when it went well, 101 or 401
//! when the call needs a login (`请先登录`). GET requests carry the pages'
//! `build_time` (milliseconds), as their request library adds it.
//!
//! The site sits behind 知道创宇's 加速乐 firewall (the `__jsluid_s`
//! cookie). When it doubts a request it answers 521 with a script that
//! sets `__jsl_clearance_s` and reloads, which a browser runs:
//! `verification_required` with the page, opened in the browser. ATP never
//! runs that script itself.
//!
//! A login is the `SXS_XSESSION_ID` cookie of shixiseng.com.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use atp_core::http::{Method, Req, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result, User, Value, ValueExt};
use jiff::Timestamp;

/// The site; links point here.
pub const WWW: &str = "https://www.shixiseng.com";
/// The gateway the login panel calls directly.
const GATEWAY: &str = "https://apigateway.shixiseng.com";
/// The cookie holding the login.
pub const SESSION: &str = "SXS_XSESSION_ID";
/// When the session cookie expires, set beside it.
const SESSION_EXPIRES: &str = "SXS_XSESSION_ID_EXP";
/// The code of answers that went well.
const OK: i64 = 100;

/// Where a call goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
  /// `www.shixiseng.com`: the search page's own endpoints.
  Site,
  /// The job and company service, through the site's proxy.
  Interns,
  /// The account service (saved jobs), through the site's proxy.
  Account,
  /// The application service, through the site's proxy.
  Deliver,
}

impl Route {
  fn base(self) -> String {
    let service = match self {
      Self::Site => return WWW.to_owned(),
      Self::Interns => "new-intern-api-host",
      Self::Account => "new-account-api-host",
      Self::Deliver => "new-deliver-api-host",
    };
    format!("{WWW}/proxy-prefix/{service}")
  }
}

pub struct Api {
  pub ctx: Ctx,
  /// Companies read in this run, by id, so the pages of a company's jobs
  /// read its page once.
  pub companies: Mutex<HashMap<String, User>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      companies: Mutex::default(),
    }
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(&[SESSION])
  }

  /// `GET` with the page's `build_time`, from the page `referer`; the
  /// whole answer (`msg` and the counts beside it). A firewall check is
  /// not retried.
  pub async fn get(
    &self,
    route: Route,
    path: &str,
    params: &[(&str, String)],
    referer: &str,
  ) -> Result<Value> {
    let req = self
      .ctx
      .http
      .get(format!("{}{path}", route.base()))
      .query("build_time", Timestamp::now().as_millisecond())
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json, text/plain, */*")
      .header("referer", referer)
      .retries(0);
    let resp = self.send(req).await?;
    self.answer(resp, referer)
  }

  /// Send a request. The site hands every visitor a session under the
  /// login's cookie name; a run without a login forgets it again, so the
  /// cookie keeps meaning a login.
  async fn send(&self, req: Req<'_>) -> Result<Resp> {
    let guest = !self.ctx.http.has_cookie(SESSION);
    let resp = req.send().await;
    if guest {
      self.ctx.http.remove_cookie(SESSION);
      self.ctx.http.remove_cookie(SESSION_EXPIRES);
    }
    resp
  }

  /// A change through the site's API (JSON, as its request library sends
  /// it), after a pause of a few seconds as a person would take.
  pub async fn post(&self, route: Route, path: &str, body: &Value, referer: &str) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    let resp = self
      .ctx
      .http
      .post(format!("{}{path}", route.base()))
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WWW)
      .header("referer", referer)
      .json(body)
      .send()
      .await?;
    self.answer(resp, referer)
  }

  /// A call of the login panel to the gateway: the JSON as it came, since
  /// the login's polling answers 500 until the code is scanned.
  pub async fn gateway(
    &self,
    method: Method,
    path: &str,
    params: &[(&str, String)],
  ) -> Result<Value> {
    let resp = self
      .ctx
      .http
      .request(method, format!("{GATEWAY}{path}"))
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WWW)
      .header("referer", format!("{WWW}/"))
      .timeout(Duration::from_secs(40))
      .send()
      .await?;
    check_firewall(&resp, WWW)?;
    serde_json::from_slice(&resp.body).map_err(|_| unexpected(&resp))
  }

  /// A file of the site (the search's font).
  pub async fn file(&self, path: &str, referer: &str) -> Result<Vec<u8>> {
    let req = self
      .ctx
      .http
      .get(format!("{WWW}{path}"))
      .header("referer", referer)
      .retries(0);
    let resp = self.send(req).await?;
    check_firewall(&resp, referer)?;
    Ok(resp.check()?.body)
  }

  /// The JSON of a response once its `code` says it went well, or the
  /// error it stands for.
  fn answer(&self, resp: Resp, page: &str) -> Result<Value> {
    check_firewall(&resp, page)?;
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      return Err(unexpected(&resp));
    };
    match v.i64("code") {
      Some(OK) => Ok(v),
      Some(code) => Err(self.error(code, &v)),
      None => Err(unexpected(&resp)),
    }
  }

  /// Shixiseng's `{code, msg}` as a shared error.
  pub fn error(&self, code: i64, v: &Value) -> Error {
    let message = v.str("msg").unwrap_or_else(|| format!("code {code}"));
    let text = format!("Shixiseng: {message}");
    match code {
      101 | 401 => {
        let what = if self.ctx.http.has_cookie(SESSION) {
          format!("{text} (the session ended)")
        } else {
          format!("{text} (this needs a login)")
        };
        Error::auth(what).with_hint(self.ctx.login_hint())
      }
      // 该职位已下线: the job was taken down.
      404 => Error::not_found(text),
      _ => Error::upstream(format!("{text} ({code})")),
    }
  }
}

/// A response that is neither the data nor a known error.
fn unexpected(resp: &Resp) -> Error {
  let text = resp.text();
  if !resp.status.is_success() {
    return status_error(resp.status, &text);
  }
  Error::upstream(format!(
    "Shixiseng answered with a page instead of data: {}",
    truncate(&one_line(&text), 160)
  ))
}

/// 加速乐's check in place of the answer: status 521, or a short page
/// whose script sets a cookie and reloads.
fn check_firewall(resp: &Resp, page: &str) -> Result<()> {
  let text = || String::from_utf8_lossy(&resp.body);
  let script = resp.body.len() < 16 * 1024
    && !resp.body.starts_with(b"{")
    && text().contains("document.cookie")
    && text().contains("location");
  if resp.status.as_u16() == 521 || script {
    return Err(
      Error::verification("Shixiseng's firewall (加速乐) wants a browser to pass its check first")
        .with_url(page),
    );
  }
  Ok(())
}
