//! Jobs and companies as arguments.
//!
//! - jobs: their number (`166040584`), or a link holding it: the job page
//!   `https://jobs.51job.com/<place>/<number>.html` (what lists print), a
//!   company site's `…/detail.html?jobid=<number>` or a mobile link with
//!   `jobid=<number>`;
//! - companies: their code (`VjlTMABlAD4CYFAwXTE`, the `encCoId` of the
//!   search), or the company page `https://jobs.51job.com/all/co<code>.html`.

use atp_core::{Error, Result};
use url::Url;

use crate::api::JOBS;

/// The job page of job `number`, under the place the search names
/// (`xian`, `xian-ytq`; `all` without one).
pub fn job_url(number: &str, place: Option<&str>) -> String {
  let place = place.filter(|p| !p.is_empty()).unwrap_or("all");
  format!("{JOBS}/{place}/{number}.html")
}

pub fn company_url(code: &str) -> String {
  format!("{JOBS}/all/co{code}.html")
}

fn is_number(text: &str) -> bool {
  (5..=12).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit())
}

/// A job's number and the page to read it on.
pub fn job(arg: &str) -> Result<(String, String)> {
  let arg = arg.trim();
  if is_number(arg) {
    return Ok((arg.to_owned(), job_url(arg, None)));
  }
  let found = Url::parse(arg).ok().and_then(|url| {
    if let Some((_, id)) = url
      .query_pairs()
      .find(|(k, _)| k.eq_ignore_ascii_case("jobid"))
      .filter(|(_, v)| is_number(v))
    {
      let url = job_url(&id, None);
      return Some((id.into_owned(), url));
    }
    let path = url.path().trim_end_matches('/');
    let (place, file) = path.trim_start_matches('/').rsplit_once('/')?;
    let number = file
      .strip_suffix(".html")
      .or_else(|| file.strip_suffix(".htm"))?;
    let ours = url.host_str() == Some("jobs.51job.com");
    (ours && is_number(number)).then(|| (number.to_owned(), job_url(number, Some(place))))
  });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a 51job job: {arg} (expected a jobs.51job.com link or the job's number)"
    ))
  })
}

/// A company's code, from its page's link or as it is.
pub fn company(arg: &str) -> Result<String> {
  let arg = arg.trim();
  let code = match Url::parse(arg) {
    Ok(url) => url
      .path()
      .rsplit('/')
      .next()
      .and_then(|file| file.strip_prefix("co"))
      .and_then(|file| file.strip_suffix(".html"))
      .map(str::to_owned),
    Err(_) => Some(arg.to_owned()),
  };
  code
    .filter(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)))
    .ok_or_else(|| {
      Error::input(format!(
        "not a 51job company: {arg} (expected a jobs.51job.com/all/co….html link or the company's code)"
      ))
    })
}
