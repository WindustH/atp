//! Jobs as arguments: the link lists print
//! (`https://job.proginn.com/d/<hash>`), the site's other job links
//! (`/job/detail/<hash>`, `.html` after it), the hash alone
//! (`5af3107aa141`) or the job's number (`recruitId`, `24897`). The
//! board's job call takes either of the two.

use atp_core::{Error, Result};
use url::Url;

use crate::api::JOB;

/// A job's page, by its hash.
pub fn job_url(hash: &str) -> String {
  format!("{JOB}/d/{hash}")
}

/// What `recruit/getRecruit` takes for a job: its number or its hash.
pub fn job(arg: &str) -> Result<String> {
  let arg = arg.trim();
  id(arg).or_else(|| from_link(arg)).ok_or_else(|| {
    Error::input(format!(
      "not a Proginn job: {arg} (expected a job.proginn.com/d/ link, the hash in it or the job's number)"
    ))
  })
}

/// A job's number (digits) or hash (eight or more hex digits).
fn id(text: &str) -> Option<String> {
  let text = text.strip_suffix(".html").unwrap_or(text);
  let number = !text.is_empty() && text.len() <= 12 && text.bytes().all(|b| b.is_ascii_digit());
  let hash = (8..=32).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit());
  (number || hash).then(|| text.to_ascii_lowercase())
}

/// The last part of a `/d/<id>` or `/job/detail/<id>` link on proginn.com.
fn from_link(arg: &str) -> Option<String> {
  let with_scheme = if arg.contains("://") {
    arg.to_owned()
  } else {
    format!("https://{arg}")
  };
  let url = Url::parse(&with_scheme).ok()?;
  let host = url.host_str()?;
  if host != "proginn.com" && !host.ends_with(".proginn.com") {
    return None;
  }
  let parts: Vec<&str> = url.path_segments()?.filter(|p| !p.is_empty()).collect();
  match parts.as_slice() {
    ["d", last] | [.., "detail", last] => id(last),
    _ => None,
  }
}
