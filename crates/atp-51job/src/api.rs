//! 51job transport. The site's pages talk to two hosts:
//!
//! - `we.51job.com`, the job search (`/api/job/search-pc`), behind
//!   Aliyun's WAF: plain HTTP gets the WAF's script check, and its slider
//!   when it doubts, so ATP asks from a page of we.51job.com in the
//!   browser tab ([`atp_core::tab`]), where the site's own scripts run,
//!   with the `sign` header those scripts add ([`sign`]);
//! - `jobs.51job.com`, job and company pages rendered with their data,
//!   behind the same WAF, read as the tab loads them ([`crate::pages`]).
//!
//! The API answers `{status, message, resultbody}`; `status` "1" is
//! success.

use atp_core::text::encode_uri_component;
use atp_core::{Ctx, Error, Result, Value, ValueExt, json};
use jiff::Timestamp;

use crate::sign::sign;

/// The search site.
pub const WE: &str = "https://we.51job.com";
/// Job and company pages.
pub const JOBS: &str = "https://jobs.51job.com";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// A GET of we.51job.com's API (`path` like `api/job/search-pc`) made by
  /// the page `from`, with the query as the site's scripts write it: its
  /// `resultbody`.
  pub async fn we(&self, path: &str, query: &[(&str, String)], from: &str) -> Result<Value> {
    let mut pairs = vec![
      ("api_key", "51job".to_owned()),
      ("timestamp", Timestamp::now().as_second().to_string()),
    ];
    pairs.extend(query.iter().cloned());
    let query: Vec<String> = pairs
      .iter()
      .map(|(key, value)| format!("{key}={}", encode_uri_component(value)))
      .collect();
    let signed = format!("/{path}?{}", query.join("&"));
    let init = json!({
      "headers": {"accept": "application/json, text/plain, */*", "sign": sign(&signed, "")},
    });
    let tab = self.ctx.tab().await?;
    let answer = tab.fetch(from, &format!("{WE}{signed}"), init).await?;
    self.result(answer.json()?)
  }

  /// The `resultbody` of an answer, or the error it stands for.
  fn result(&self, v: Value) -> Result<Value> {
    if v.str("status").as_deref() == Some("1") {
      return Ok(v.at("resultbody").clone());
    }
    let message = v
      .first_str(&["message", "msg"])
      .unwrap_or_else(|| "no message".into());
    Err(Error::upstream(format!(
      "51job: {message} (status {})",
      v.str("status").unwrap_or_default()
    )))
  }
}
