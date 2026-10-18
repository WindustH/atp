//! Jobs, companies and chats as arguments.
//!
//! - jobs: the job page links lists print, `https://www.liepin.com/a/<n>.shtml`
//!   (jobs posted by headhunters) and `https://www.liepin.com/job/<n>.shtml`
//!   (jobs posted by companies; `m.liepin.com` links too), `/lptjob/<n>`
//!   links of company pages, `a/<n>` and `job/<n>`, or the number of the
//!   link alone: company jobs have ten digits beginning with 19
//!   (`1985552821`), headhunter jobs fewer (`78980551`);
//! - companies: `https://www.liepin.com/company/<n>/` (and the
//!   `company-jobs` page), `company/<n>`, or the number;
//! - chats: the id `chats` lists (the other side's IM id).

use atp_core::{Error, Result};
use url::Url;

use crate::api::WWW;

/// A job page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRef {
  /// `/a/<n>.shtml`: posted by a headhunter (job kind 1).
  Headhunter(String),
  /// `/job/<n>.shtml`: posted by a company.
  Company(String),
  /// `/lptjob/<n>`: a company page's link to one of its jobs.
  Lpt(String),
}

impl JobRef {
  pub fn url(&self) -> String {
    match self {
      Self::Headhunter(n) => format!("{WWW}/a/{n}.shtml"),
      Self::Company(n) => format!("{WWW}/job/{n}.shtml"),
      Self::Lpt(n) => format!("{WWW}/lptjob/{n}"),
    }
  }
}

/// The path of a link (or of `a/<n>` as typed), without the slashes
/// around it, the query and the `#` part.
fn path_of(arg: &str) -> String {
  let arg = arg.trim();
  match Url::parse(arg) {
    Ok(url) => url.path().trim_matches('/').to_owned(),
    Err(_) => arg
      .split(['#', '?'])
      .next()
      .unwrap_or(arg)
      .trim_matches('/')
      .to_owned(),
  }
}

fn digits(text: &str) -> Option<String> {
  (!text.is_empty() && text.len() <= 16 && text.bytes().all(|b| b.is_ascii_digit()))
    .then(|| text.to_owned())
}

/// The number of a job link (`a/78980551.shtml` → `78980551`).
fn number(rest: &str) -> Option<String> {
  digits(rest.trim_end_matches(".shtml").split('/').next()?)
}

pub fn job(arg: &str) -> Result<JobRef> {
  let path = path_of(arg);
  let parsed = if let Some(rest) = path.strip_prefix("a/") {
    number(rest).map(JobRef::Headhunter)
  } else if let Some(rest) = path.strip_prefix("job/") {
    number(rest).map(JobRef::Company)
  } else if let Some(rest) = path.strip_prefix("lptjob/") {
    number(rest).map(JobRef::Lpt)
  } else {
    digits(&path).map(|n| {
      if n.len() >= 10 && n.starts_with("19") {
        JobRef::Company(n)
      } else {
        JobRef::Headhunter(n)
      }
    })
  };
  parsed.ok_or_else(|| {
    Error::input(format!(
      "not a Liepin job: {arg} (expected a /job/ or /a/ link, or the number in it)"
    ))
  })
}

/// The number of a job's page link, which lists print as the job's id.
pub fn job_id(link: &str) -> Option<String> {
  match job(link).ok()? {
    JobRef::Headhunter(n) | JobRef::Company(n) | JobRef::Lpt(n) => Some(n),
  }
}

pub fn company(arg: &str) -> Result<String> {
  let path = path_of(arg);
  let id = ["company/", "company-jobs/"]
    .iter()
    .find_map(|prefix| path.strip_prefix(prefix))
    .unwrap_or(&path);
  id.split('/').next().and_then(digits).ok_or_else(|| {
    Error::input(format!(
      "not a Liepin company: {arg} (expected a /company/ link or its number)"
    ))
  })
}

pub fn company_url(id: &str) -> String {
  format!("{WWW}/company/{id}/")
}

/// A chat: the other side's IM id (32 hex digits), as `chats` lists it.
pub fn chat(arg: &str) -> Result<String> {
  let id = arg.trim();
  if (16..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!(
      "not a Liepin chat: {arg} (expected an id from `chats`)"
    )))
  }
}
