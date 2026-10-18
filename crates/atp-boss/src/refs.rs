//! Jobs, companies and chats as arguments.
//!
//! - jobs: their id (`af620b46e682c71f0nZ409q8FVNV`, the `encryptJobId`
//!   of the site's cards), or the job page
//!   `https://www.zhipin.com/job_detail/<id>.html` (what lists print;
//!   `m.zhipin.com` links too). A link the site's own cards make carries
//!   `securityId` and `lid` in its query, which actions on the job reuse;
//! - companies: their id (`ed38088b0f836f9e1Xd80t64EQ~~`, the
//!   `encryptBrandId`), or the company page
//!   `https://www.zhipin.com/gongsi/<id>.html` (or `/gongsi/job/<id>.html`);
//! - chats: the number the chat list gives them (the recruiter's account
//!   number, `friendId`).

use atp_core::text::encode_uri_component;
use atp_core::{Error, Result};
use url::Url;

use crate::api::WWW;

/// A job as an argument: its id and what the link it came in carried.
#[derive(Debug, Clone, Default)]
pub struct JobRef {
  pub id: String,
  pub security_id: Option<String>,
  pub lid: Option<String>,
}

impl JobRef {
  /// The job's page.
  pub fn url(&self) -> String {
    job_url(&self.id)
  }
}

pub fn job_url(id: &str) -> String {
  format!("{WWW}/job_detail/{id}.html")
}

/// The job page as the site's own cards link it, with the `securityId`
/// and `lid` actions on the job take.
pub fn job_link(id: &str, security_id: Option<&str>, lid: Option<&str>) -> String {
  let Some(security_id) = security_id.filter(|s| !s.is_empty()) else {
    return job_url(id);
  };
  let mut link = format!(
    "{}?securityId={}",
    job_url(id),
    encode_uri_component(security_id)
  );
  if let Some(lid) = lid.filter(|l| !l.is_empty()) {
    link.push_str(&format!("&lid={}", encode_uri_component(lid)));
  }
  link
}

pub fn company_url(id: &str) -> String {
  format!("{WWW}/gongsi/{id}.html")
}

/// The page listing a company's jobs.
pub fn company_jobs_url(id: &str) -> String {
  format!("{WWW}/gongsi/job/{id}.html")
}

/// Whether `text` looks like one of the site's encrypted ids.
fn is_id(text: &str) -> bool {
  (10..=64).contains(&text.len())
    && text
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b"_-~".contains(&b))
}

/// A link of the site, if `arg` is one.
fn ours(arg: &str) -> Option<Url> {
  let url = Url::parse(arg).ok()?;
  let host = url.host_str()?;
  (host == "zhipin.com" || host.ends_with(".zhipin.com")).then_some(url)
}

/// The id in a page's file name: `/job_detail/<id>.html` → `<id>`.
fn file_id<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
  path
    .strip_prefix(prefix)?
    .strip_suffix(".html")
    .filter(|id| is_id(id))
}

pub fn job(arg: &str) -> Result<JobRef> {
  let arg = arg.trim();
  if is_id(arg) {
    return Ok(JobRef {
      id: arg.to_owned(),
      ..JobRef::default()
    });
  }
  let found = ours(arg).and_then(|url| {
    let id = file_id(url.path(), "/job_detail/")?.to_owned();
    let query = |key: &str| {
      url
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .filter(|v| !v.is_empty())
    };
    Some(JobRef {
      id,
      security_id: query("securityId"),
      lid: query("lid"),
    })
  });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a BOSS Zhipin job: {arg} (expected a zhipin.com/job_detail/….html link or the job's id)"
    ))
  })
}

pub fn company(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if is_id(arg) {
    return Ok(arg.to_owned());
  }
  let found = ours(arg).and_then(|url| {
    let path = url.path();
    file_id(path, "/gongsi/job/")
      .or_else(|| file_id(path, "/gongsi/"))
      .map(str::to_owned)
  });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a BOSS Zhipin company: {arg} (expected a zhipin.com/gongsi/….html link or the company's id)"
    ))
  })
}

pub fn chat(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  Err(Error::input(format!(
    "not a BOSS Zhipin chat: {arg} (expected the number `chats` lists, or #N)"
  )))
}
