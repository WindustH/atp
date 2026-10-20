//! SMZDM transport. Two ways in:
//!
//! - plain HTTP for what answers it: the front page's list
//!   (`www.smzdm.com/homepage/json_more`, JSON as the page's scroll asks
//!   for it, `{error_code, data}`) and the pages of deals
//!   (`www.smzdm.com/p/<id>/`) and articles (`post.smzdm.com/p/<id>/`),
//!   read as a browser loads them;
//! - a browser tab ([`atp_core::tab`]) for the search
//!   (`search.smzdm.com`), which answers plain requests with a script
//!   check (`probe.js`) that only a browser passes: a page of the search
//!   site makes the request, after its scripts have passed the check.
//!   The site may show a browser Tencent's captcha instead, which is
//!   `verification_required` for the person to pass.
//!
//! Nothing needs an account. A check where a page was expected, HTTP 429
//! and pages without their data become `verification_required`,
//! `rate_limited` and `upstream_error`; nothing is retried.

use atp_core::http::{Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};

/// The site.
pub const WWW: &str = "https://www.smzdm.com";
/// The search site.
pub const SEARCH: &str = "https://search.smzdm.com";
/// What marks the site's checks: its script check, and the Tencent
/// captcha it may show instead.
const CHECKS: &[&str] = &["/probe.js?v=", "new TencentCaptcha("];

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// The HTML of a page of the site.
  pub async fn page(&self, url: &str) -> Result<String> {
    let resp = self
      .ctx
      .http
      .get(url)
      .retries(0)
      .header(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      )
      .header("referer", format!("{WWW}/"))
      .send()
      .await?;
    self.html(resp, url)
  }

  fn html(&self, resp: Resp, url: &str) -> Result<String> {
    let text = resp.text();
    let head = text.get(..16_384).unwrap_or(&text);
    if CHECKS.iter().any(|mark| head.contains(mark)) {
      return Err(checked(url));
    }
    match resp.status.as_u16() {
      200..=299 => Ok(text),
      404 => Err(Error::not_found(format!("SMZDM has no page {url}"))),
      429 => Err(rate_limited()),
      _ => Err(status_error(resp.status, &text)),
    }
  }

  /// `GET <WWW><path>?<params>` of the JSON the site's pages ask for.
  pub async fn json(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    let url = format!("{WWW}{path}");
    let resp = self
      .ctx
      .http
      .get(&url)
      .retries(0)
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json, text/javascript, */*; q=0.01")
      .header("x-requested-with", "XMLHttpRequest")
      .header("referer", format!("{WWW}/"))
      .send()
      .await?;
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      let html = self.html(resp, &url)?;
      return Err(Error::upstream(format!(
        "SMZDM answered with a page instead of data: {}",
        truncate(&one_line(&html), 160)
      )));
    };
    match v.i64("error_code") {
      Some(0) | None => Ok(v),
      Some(code) => Err(Error::upstream(format!(
        "SMZDM: {} (error {code})",
        v.str("error_msg").unwrap_or_else(|| "no message".into())
      ))),
    }
  }

  /// A page of the search site, asked by a page of it in the browser tab.
  pub async fn search_page(&self, url: &str) -> Result<String> {
    let tab = self.ctx.tab().await?;
    let init = json!({"headers": {"accept": "text/html,application/xhtml+xml,*/*;q=0.8"}});
    let answer = tab.fetch(&format!("{SEARCH}/"), url, init).await?;
    answer.passed()?;
    match answer.status {
      200..=299 => Ok(answer.body),
      429 => Err(rate_limited()),
      status => Err(Error::upstream(format!(
        "SMZDM's search answered HTTP {status}"
      ))),
    }
  }
}

/// The site's check where a page was expected. Plain requests carry no
/// cookies a browser pass could set, so ATP does not open it.
fn checked(url: &str) -> Error {
  Error::verification("SMZDM's check (a script check or a captcha) stopped the request")
    .with_url(url)
    .with_hint("retry later, or from another network")
    .unclearable()
}

fn rate_limited() -> Error {
  Error::new(
    ErrorCode::RateLimited,
    "SMZDM refused the request (HTTP 429, too many requests)",
  )
  .with_hint("wait a while before retrying, and ask for fewer pages at a time")
}
