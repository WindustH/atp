//! Jobs and companies as arguments.
//!
//! - jobs: the link lists print (`https://www.linkedin.com/jobs/view/<id>`),
//!   any job link as LinkedIn writes it (`/jobs/view/<title>-at-<company>-<id>`,
//!   on country hosts such as `br.linkedin.com` too), links of search and
//!   collection pages naming the job open beside them
//!   (`?currentJobId=<id>`), `urn:li:jobPosting:<id>` (and
//!   `urn:li:fsd_jobPosting:<id>`), or the id alone;
//! - companies: `https://www.linkedin.com/company/<name>` (any page under
//!   it, such as `/jobs`), the name in that link, the company's number
//!   (`company_id` of a job page), or `urn:li:company:<n>` /
//!   `urn:li:organization:<n>` / `urn:li:fsd_company:<n>`.

use std::sync::LazyLock;

use atp_core::{Error, Result};
use regex::Regex;
use url::Url;

use crate::api::WWW;

static JOB_URN: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?i)^urn:li:(?:fsd_|fs_normalized_)?jobposting:(\d+)$").expect("valid pattern")
});
static COMPANY_URN: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?i)^urn:li:(?:fsd_|fs_normalized_)?(?:company|organization):(\d+)$")
    .expect("valid pattern")
});
static NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[\w][\w.%-]*$").expect("valid pattern"));

/// The link of a job, as lists print it.
pub fn job_url(id: &str) -> String {
  format!("{WWW}/jobs/view/{id}")
}

/// A job's id: digits only.
fn job_id(text: &str) -> Option<String> {
  (text.len() >= 5 && text.len() <= 20 && text.bytes().all(|b| b.is_ascii_digit()))
    .then(|| text.to_owned())
}

/// A link on LinkedIn, `https://` optional.
fn link(arg: &str) -> Option<Url> {
  let with_scheme = if arg.contains("://") {
    arg.to_owned()
  } else {
    format!("https://{arg}")
  };
  let url = Url::parse(&with_scheme).ok()?;
  let host = url.host_str()?;
  (host == "linkedin.com" || host.ends_with(".linkedin.com")).then_some(url)
}

/// The segment after `marker` in a link's path (`/view/<x>`).
fn after<'a>(url: &'a Url, marker: &str) -> Option<&'a str> {
  let mut parts = url.path_segments()?.filter(|p| !p.is_empty());
  parts.find(|p| *p == marker)?;
  parts.next()
}

/// A job's id from its link, its URN or the id alone.
pub fn job(arg: &str) -> Result<String> {
  let arg = arg.trim();
  let found = job_id(arg)
    .or_else(|| JOB_URN.captures(arg).map(|c| c[1].to_owned()))
    .or_else(|| {
      let url = link(arg)?;
      let current = url
        .query_pairs()
        .find(|(k, _)| k == "currentJobId")
        .and_then(|(_, v)| job_id(&v));
      // `/jobs/view/<id>`, `/jobs/view/<title>-<id>`, the guest API's `/jobPosting/<id>`.
      let viewed = after(&url, "view")
        .or_else(|| after(&url, "jobPosting"))
        .and_then(|part| job_id(part.rsplit('-').next().unwrap_or(part)));
      current.or(viewed)
    });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a LinkedIn job: {arg} (expected a /jobs/view/ link, urn:li:jobPosting:<id> or the id)"
    ))
  })
}

/// A company, by its number or by the name in its link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanyRef {
  /// The number LinkedIn's job search filters by (`f_C`).
  Id(String),
  /// The name in `/company/<name>`.
  Name(String),
}

impl CompanyRef {
  /// The company's page (LinkedIn sends `/company/<n>` on to the name).
  pub fn url(&self) -> String {
    match self {
      Self::Id(id) | Self::Name(id) => format!("{WWW}/company/{id}"),
    }
  }
}

pub fn company(arg: &str) -> Result<CompanyRef> {
  let arg = arg.trim();
  let part = if let Some(c) = COMPANY_URN.captures(arg) {
    Some(c[1].to_owned())
  } else if arg.contains('/') {
    link(arg).and_then(|url| after(&url, "company").map(str::to_owned))
  } else {
    Some(arg.to_owned())
  };
  match part {
    Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => Ok(CompanyRef::Id(p)),
    Some(p) if NAME.is_match(&p) => Ok(CompanyRef::Name(p)),
    _ => Err(Error::input(format!(
      "not a LinkedIn company: {arg} (expected a /company/ link, the name in it, or the company's number)"
    ))),
  }
}

/// The name in a company link (`https://www.linkedin.com/company/nvidia?trk=…`).
pub fn company_name(link_text: &str) -> Option<String> {
  match company(link_text).ok()? {
    CompanyRef::Name(name) | CompanyRef::Id(name) => Some(name),
  }
}
