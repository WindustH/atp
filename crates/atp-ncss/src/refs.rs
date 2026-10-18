//! Jobs, companies and job fairs as arguments.
//!
//! - jobs: `https://www.ncss.cn/student/jobs/<id>/detail.html` (what lists
//!   print; `job.ncss.cn` and the province and school sites under
//!   `ncss.cn` too), `jobs/<id>`, or the id (`MprVzHq4QAhDGX1XiSpy1k`);
//! - companies: `https://www.ncss.cn/student/jobs/<id>/corp.html`, or the
//!   id of 32 hex digits;
//! - job fairs: `https://www.ncss.cn/student/jobfair/fairdetails.html?fairId=<id>`
//!   (what `fairs` prints), the mobile `jobfair/jobs.html?fairId=<id>`, or
//!   the id.

use atp_core::{Error, Result};
use url::Url;

use crate::api::WWW;

pub fn job_url(id: &str) -> String {
  format!("{WWW}/student/jobs/{id}/detail.html")
}

pub fn company_url(id: &str) -> String {
  format!("{WWW}/student/jobs/{id}/corp.html")
}

pub fn fair_url(id: &str) -> String {
  format!("{WWW}/student/jobfair/fairdetails.html?fairId={id}")
}

/// An id as the site writes them: letters and digits.
fn is_id(text: &str) -> bool {
  (8..=40).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A company id: 32 hex digits.
fn is_company(text: &str) -> bool {
  text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The parts of a link's path (or of `jobs/<id>` as typed) and its query.
fn split(arg: &str) -> (Vec<String>, Option<Url>) {
  let arg = arg.trim();
  if let Ok(url) = Url::parse(arg) {
    let parts = url
      .path_segments()
      .map(|s| s.filter(|p| !p.is_empty()).map(str::to_owned).collect())
      .unwrap_or_default();
    return (parts, Some(url));
  }
  let path = arg.split(['#', '?']).next().unwrap_or(arg);
  let parts = path
    .split('/')
    .filter(|p| !p.is_empty())
    .map(str::to_owned)
    .collect();
  (parts, None)
}

/// A job's id, from its link or as it is.
pub fn job(arg: &str) -> Result<String> {
  let (parts, _) = split(arg);
  let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
  let id = match parts.as_slice() {
    [.., "jobs", id, "detail.html"] | ["jobs", id] => Some(*id),
    [id] if is_id(id) && !is_company(id) => Some(*id),
    _ => None,
  };
  id.filter(|id| is_id(id)).map(str::to_owned).ok_or_else(|| {
    Error::input(format!(
      "not an NCSS job: {arg} (expected a /student/jobs/<id>/detail.html link or the job's id)"
    ))
  })
}

/// A company's id, from its page's link or as it is.
pub fn company(arg: &str) -> Result<String> {
  let (parts, _) = split(arg);
  let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
  let id = match parts.as_slice() {
    [.., "jobs", id, "corp.html"] => Some(*id),
    [id] => Some(*id),
    _ => None,
  };
  id.filter(|id| is_company(id))
    .map(str::to_lowercase)
    .ok_or_else(|| {
      Error::input(format!(
        "not an NCSS company: {arg} (expected a /student/jobs/<id>/corp.html link or its id of 32 hex digits)"
      ))
    })
}

/// A job fair's id, from its link (`?fairId=`) or as it is.
pub fn fair(arg: &str) -> Result<String> {
  let (parts, url) = split(arg);
  let from_query = url.and_then(|u| {
    u.query_pairs()
      .find(|(k, _)| k == "fairId")
      .map(|(_, v)| v.into_owned())
  });
  let id = from_query.or_else(|| match parts.as_slice() {
    [id] => Some(id.clone()),
    _ => None,
  });
  id.filter(|id| is_id(id)).ok_or_else(|| {
    Error::input(format!(
      "not an NCSS job fair: {arg} (expected a fairdetails.html?fairId= link or the fair's id)"
    ))
  })
}
