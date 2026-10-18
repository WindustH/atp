//! NCSS transport. The student side of `www.ncss.cn` is pages rendered on
//! the server (jobs, companies, job fairs) and the JSON their scripts load
//! with jQuery (`X-Requested-With: XMLHttpRequest`):
//!
//! - lists answer `{flag, data, global, errors}`: `flag` true with `data`,
//!   or false with the reason in `global[0].des`; `请登录后查看` asks for a
//!   login (the job search past its tenth page);
//! - some answer bare (`{total, jobList}` for fairs, an array for codes).
//!
//! Changes to the account (saving, applying) are form posts carrying a
//! token of the page that offers them (`<input id="csrf-…" name="cprmcsrf">`).
//! Pages say whether a student is logged in (`#mask4`, `#maskUser`).
//!
//! A login is the session of a 学信网 (CHSI) account, which the site keeps
//! in its cookies; ATP takes it from a browser.

use std::time::Duration;

use atp_core::http::{Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result, Value, ValueExt};

use crate::parse;

/// The site; links point here.
pub const WWW: &str = "https://www.ncss.cn";
/// What the site answers when a list wants a login.
const NEED_LOGIN: &str = "请登录后查看";
/// The cookie of the site's session (`Path=/student/`), which a login
/// fills: every visit gets one, so it says nothing about the login itself.
pub const SESSION: &str = "SESSION";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// `GET` JSON as the pages' scripts load it; the whole answer.
  pub async fn json(&self, path: &str, params: &[(&str, String)], referer: &str) -> Result<Value> {
    let resp = self
      .ctx
      .http
      .get(format!("{WWW}{path}"))
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json, text/javascript, */*; q=0.01")
      .header("x-requested-with", "XMLHttpRequest")
      .header("referer", format!("{WWW}{referer}"))
      .send()
      .await?;
    self.value(resp, referer)
  }

  /// `GET` a `{flag, data}` answer; its `data`.
  pub async fn data(&self, path: &str, params: &[(&str, String)], referer: &str) -> Result<Value> {
    let v = self.json(path, params, referer).await?;
    self.flagged(v)
  }

  /// A change to the account: a form posted from the page at `referer`,
  /// after a pause of a few seconds as a person would take.
  pub async fn write(&self, path: &str, form: &[(&str, String)], referer: &str) -> Result<Value> {
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    let resp = self
      .ctx
      .http
      .post(format!("{WWW}{path}"))
      .header("accept", "application/json, text/javascript, */*; q=0.01")
      .header("x-requested-with", "XMLHttpRequest")
      .header("origin", WWW)
      .header("referer", format!("{WWW}{referer}"))
      .form(form.iter().map(|(k, v)| (*k, v.as_str())))
      .send()
      .await?;
    self.value(resp, referer)
  }

  /// A page of the site as HTML.
  pub async fn page(&self, path: &str) -> Result<String> {
    let url = format!("{WWW}{path}");
    let resp = self
      .ctx
      .http
      .get(&url)
      .header(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      )
      .header("referer", format!("{WWW}/student/jobs/index.html"))
      .send()
      .await?;
    if resp.status.as_u16() == 404 {
      return Err(Error::not_found(format!("NCSS has no page {url}")));
    }
    let html = resp.check()?.text();
    check_waf(&html, &url)?;
    Ok(html)
  }

  /// A page of the site, read as the logged-in student: fails when the
  /// page says no student is logged in.
  pub async fn own_page(&self, path: &str) -> Result<String> {
    let html = self.page(path).await?;
    match parse::login_state(&html) {
      Some(parse::LoginState::Student) => Ok(html),
      Some(parse::LoginState::Other) => Err(Error::auth(
        "NCSS: the session is an employer's or a manager's account, not a student's",
      )),
      _ => Err(self.login_error("NCSS: no student is logged in")),
    }
  }

  /// The JSON of a response, or the error it stands for.
  fn value(&self, resp: Resp, referer: &str) -> Result<Value> {
    if let Ok(v) = serde_json::from_slice::<Value>(&resp.body) {
      return Ok(v);
    }
    let text = resp.text();
    check_waf(&text, &format!("{WWW}{referer}"))?;
    if !resp.status.is_success() {
      return Err(status_error(resp.status, &text));
    }
    Err(Error::upstream(format!(
      "NCSS answered with a page instead of data: {}",
      truncate(&one_line(&text), 160)
    )))
  }

  /// The `data` of a `{flag, data}` answer.
  pub fn flagged(&self, v: Value) -> Result<Value> {
    if v.bool("flag") == Some(true) {
      return Ok(v.at("data").clone());
    }
    let reason = v
      .first_str(&["global.0.des", "errors.0.des", "msg"])
      .unwrap_or_else(|| "the request was refused".into());
    if reason.contains(NEED_LOGIN) {
      return Err(self.login_error(&format!("NCSS: {reason}")));
    }
    Err(Error::upstream(format!("NCSS: {reason}")))
  }

  /// `not_authenticated`, with the way to log in.
  pub fn login_error(&self, text: &str) -> Error {
    Error::auth(format!(
      "{text} (this needs a student's login; a saved one may have ended)"
    ))
    .with_hint(self.ctx.login_hint())
  }
}

/// The firewall's check page (Aliyun WAF) in place of an answer.
fn check_waf(text: &str, url: &str) -> Result<()> {
  if text.contains("aliyun_waf") || text.contains("acw_sc__v2") || text.contains("_waf_") {
    return Err(
      Error::verification("NCSS's firewall wants a check (滑块验证) before going on").with_url(url),
    );
  }
  Ok(())
}
