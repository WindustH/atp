//! Jobs and companies as arguments.
//!
//! - jobs: their number (`CC000638920J40889851614`: the company's, `J`,
//!   the job's), in the links lists print (`https://m.zhaopin.com/jobs/<n>.htm`),
//!   `https://jobs.zhaopin.com/<n>.htm` and
//!   `https://www.zhaopin.com/jobdetail/<n>.htm`;
//! - companies: their number (`CZ000638920`, `CZL1337269640`), in
//!   `https://m.zhaopin.com/company/<n>.htm`,
//!   `https://company.zhaopin.com/<n>.htm` and
//!   `https://www.zhaopin.com/companydetail/<n>.htm`.

use atp_core::{Error, Result};
use url::Url;

use crate::api::M;

pub fn job_url(number: &str) -> String {
  format!("{M}/jobs/{number}.htm")
}

pub fn company_url(number: &str) -> String {
  format!("{M}/company/{number}.htm")
}

/// The last part of a link's path without `.htm` (or the argument as
/// typed).
fn last_part(arg: &str) -> String {
  let arg = arg.trim();
  let path = match Url::parse(arg) {
    Ok(url) => url.path().to_owned(),
    Err(_) => arg.split(['#', '?']).next().unwrap_or(arg).to_owned(),
  };
  let last = path.trim_matches('/').rsplit('/').next().unwrap_or("");
  last
    .trim_end_matches(".html")
    .trim_end_matches(".htm")
    .to_owned()
}

/// `CC…J…`: letters, then digits, `J` and digits.
fn is_job(text: &str) -> bool {
  let Some((company, job)) = text.split_once('J') else {
    return false;
  };
  company.starts_with('C')
    && company.len() >= 4
    && company.bytes().all(|b| b.is_ascii_alphanumeric())
    && !job.is_empty()
    && job.bytes().all(|b| b.is_ascii_digit())
}

/// `CZ…`: `CZ`, maybe `L`, digits.
fn is_company(text: &str) -> bool {
  let digits = text
    .strip_prefix("CZL")
    .or_else(|| text.strip_prefix("CZ"))
    .unwrap_or("");
  !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// A job's number, from its link or as it is.
pub fn job(arg: &str) -> Result<String> {
  let number = last_part(arg).to_uppercase();
  if is_job(&number) {
    return Ok(number);
  }
  Err(Error::input(format!(
    "not a Zhilian job: {arg} (expected a /jobs/ link or the job's number, CC…J…)"
  )))
}

/// A company's number, from its link or as it is.
pub fn company(arg: &str) -> Result<String> {
  let number = last_part(arg).to_uppercase();
  if is_company(&number) {
    return Ok(number);
  }
  Err(Error::input(format!(
    "not a Zhilian company: {arg} (expected a /company/ link or the company's number, CZ…)"
  )))
}
