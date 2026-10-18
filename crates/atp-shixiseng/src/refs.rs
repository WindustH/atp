//! Jobs and companies as arguments.
//!
//! - jobs: their id (`inn_l1azdlkp0wof`), or a link holding it:
//!   `https://www.shixiseng.com/intern/<id>` (what lists print), the
//!   mobile site's links and shared links with `?utm_…`;
//! - companies: their id (`com_qn76ufrrerod`), or a link holding it:
//!   `https://www.shixiseng.com/com/<id>` (what lists print).

use atp_core::{Error, Result};
use url::Url;

use crate::api::WWW;

pub fn job_url(id: &str) -> String {
  format!("{WWW}/intern/{id}")
}

pub fn company_url(id: &str) -> String {
  format!("{WWW}/com/{id}")
}

/// The part of a link's path (or of the argument as typed) that starts
/// with `prefix` and holds letters and digits only after it.
fn id(arg: &str, prefix: &str) -> Option<String> {
  let arg = arg.trim();
  let path = match Url::parse(arg) {
    Ok(url) => url.path().to_owned(),
    Err(_) => arg.split(['#', '?']).next().unwrap_or(arg).to_owned(),
  };
  path
    .split('/')
    .map(|part| part.trim_end_matches(".html"))
    .find(|part| {
      part
        .strip_prefix(prefix)
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric()))
    })
    .map(str::to_owned)
}

/// A job's id, from its link or as it is.
pub fn job(arg: &str) -> Result<String> {
  id(arg, "inn_").ok_or_else(|| {
    Error::input(format!(
      "not a Shixiseng job: {arg} (expected an /intern/ link or the job's id, inn_…)"
    ))
  })
}

/// A company's id, from its link or as it is.
pub fn company(arg: &str) -> Result<String> {
  id(arg, "com_").ok_or_else(|| {
    Error::input(format!(
      "not a Shixiseng company: {arg} (expected a /com/ link or the company's id, com_…)"
    ))
  })
}
