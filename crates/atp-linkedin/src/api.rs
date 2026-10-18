//! LinkedIn transport: the public pages of `www.linkedin.com` that its
//! jobs pages show a visitor without an account. The job search and job
//! pages are HTML fragments of the guest API (`/jobs-guest/jobs/api/…`)
//! the pages load as one scrolls; companies are their public pages
//! (`/company/<name>`). Everything is asked for in English, so the
//! parsers read the same words wherever the request comes from.
//!
//! LinkedIn answers visitors who come too often with HTTP 429 or its own
//! HTTP 999, or sends them to its auth wall (`/authwall`), a sign-in page
//! or a security check (`/checkpoint/`). Those become `rate_limited`,
//! `not_authenticated` and `verification_required` here, and the page is
//! never parsed in place of the one asked for. Nothing is retried: a
//! refusal is not worth repeating to a site that bans for it.
//!
//! The cookies LinkedIn hands a visitor live for one command and are
//! never saved.

use atp_core::http::{Resp, status_error};
use atp_core::text::{html_to_text, one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result};
use url::Url;

/// The site.
pub const WWW: &str = "https://www.linkedin.com";
/// The guest API of the jobs pages.
pub const GUEST: &str = "https://www.linkedin.com/jobs-guest/jobs/api";
/// What browsers send for a page.
const HTML: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
/// English, whatever the person's own language.
const LANGUAGE: &str = "en-US,en;q=0.9";
/// Redirects followed before giving up (`/company/<n>` → `/company/<name>`).
const MAX_HOPS: usize = 3;

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// A page as HTML, `None` when LinkedIn has none there (404, 410).
  /// Redirects within LinkedIn are followed; one to its auth wall, a
  /// sign-in page or a security check is an error.
  pub async fn html(&self, url: &str, query: &[(&str, String)]) -> Result<Option<String>> {
    let mut url = Url::parse(url).map_err(|e| Error::internal(format!("bad url {url}: {e}")))?;
    if !query.is_empty() {
      url.query_pairs_mut().extend_pairs(query);
    }
    for _ in 0..=MAX_HOPS {
      let resp = self
        .ctx
        .http
        .get(url.as_str())
        .header("accept", HTML)
        .header("accept-language", LANGUAGE)
        .no_redirects()
        .retries(0)
        .send()
        .await?;
      if resp.status.is_redirection() {
        url = next_hop(&url, &resp)?;
        continue;
      }
      return page(resp);
    }
    Err(Error::upstream(format!(
      "LinkedIn kept redirecting {url}; try again later"
    )))
  }
}

/// Where a redirect leads, unless it is one of LinkedIn's walls or leaves
/// LinkedIn.
fn next_hop(from: &Url, resp: &Resp) -> Result<Url> {
  let location = resp.header("location").unwrap_or_default();
  let to = from
    .join(location)
    .map_err(|_| Error::upstream(format!("LinkedIn redirected to {location}")))?;
  let path = to.path();
  if WALLS.iter().any(|wall| path.starts_with(wall)) {
    return Err(walled());
  }
  if path.starts_with(CHECK) {
    return Err(checked(to.as_str()));
  }
  let host = to.host_str().unwrap_or_default();
  if host != "linkedin.com" && !host.ends_with(".linkedin.com") {
    return Err(Error::upstream(format!(
      "LinkedIn redirected {from} away from LinkedIn, to {to}"
    )));
  }
  Ok(to)
}

/// The HTML of an answer, or the error its status or page stands for.
fn page(resp: Resp) -> Result<Option<String>> {
  match resp.status.as_u16() {
    404 | 410 => return Ok(None),
    429 => return Err(refused("HTTP 429, too many requests")),
    999 => return Err(refused("HTTP 999, its answer to suspected automation")),
    _ => {}
  }
  if !resp.status.is_success() {
    return Err(status_error(resp.status, &resp.text()));
  }
  let text = resp.text();
  // Pages ATP reads never name these; the wall and the check do.
  if text.contains(CHALLENGE) {
    return Err(checked(&format!("{WWW}/jobs/")));
  }
  if text.contains("authwall") {
    return Err(walled());
  }
  Ok(Some(text))
}

/// Where LinkedIn sends visitors it wants signed in.
const WALLS: &[&str] = &["/authwall", "/login", "/uas/login", "/signup"];
/// Where it sends visitors it wants to check.
const CHECK: &str = "/checkpoint/";
/// What a check's page links to.
const CHALLENGE: &str = "/checkpoint/challenge";

/// The auth wall or a sign-in page instead of the page asked for.
fn walled() -> Error {
  Error::auth(
    "LinkedIn put up its sign-in wall instead of the page, as it does for visitors who come often",
  )
  .with_hint(
    "ATP reads LinkedIn without an account; wait a while (an hour or more) before retrying",
  )
}

/// A security check (captcha) instead of the page asked for; requests
/// without an account carry nothing a browser pass could clear.
fn checked(page: &str) -> Error {
  Error::verification("LinkedIn wants a security check before showing this; ATP does not pass it")
    .with_url(page)
    .unclearable()
}

/// LinkedIn refusing requests from this address.
fn refused(why: &str) -> Error {
  Error::new(
    ErrorCode::RateLimited,
    format!("LinkedIn refused the request ({why})"),
  )
  .with_hint("wait a while (an hour or more) before retrying, and ask for fewer pages at a time")
}

/// A page that does not hold what it should, as an error naming what it
/// holds instead.
pub fn unexpected(what: &str, html: &str) -> Error {
  let start: String = html.chars().take(4000).collect();
  Error::upstream(format!(
    "LinkedIn answered with a page that holds no {what}: {}",
    truncate(&one_line(&html_to_text(&start)), 160)
  ))
}
