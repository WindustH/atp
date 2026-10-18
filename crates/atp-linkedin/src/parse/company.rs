//! A company's public page (`/company/<name>`): the top card (name,
//! industry, tagline, logo, the "See jobs" link with the company numbers
//! its jobs are searched by), the followers in the page's description,
//! the company's own number (`urn:li:organization:<n>`) and "About us"
//! (description, website, industry, size, headquarters, type, founded,
//! specialties).
//!
//! The page also lists employees, people who follow it, its posts and
//! similar pages; none of that is read.

use std::sync::LazyLock;

use atp_core::text::parse_count;
use atp_core::{User, UserStats, Value, json};
use regex::Regex;

use super::{attr, capture, line, re, read_parts};
use crate::refs;

static NAME: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h1 class="top-card-layout__title[^"]*">(.*?)</h1>"#));
static HEADLINE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h2 class="top-card-layout__headline[^"]*">(.*?)</h2>"#));
static TAGLINE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h4 class="top-card-layout__second-subline[^"]*">(.*?)</h4>"#));
static CANONICAL: LazyLock<Regex> = LazyLock::new(|| re(r#"<link rel="canonical" href="([^"]+)""#));
static IMAGE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<meta property="og:image" content="([^"]+)""#));
static FOLLOWERS: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<meta name="description" content="[^"|]*\|\s*([\d,]+) followers"#));
static ORGANIZATION: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)<code id="flagshipOrganizationTracking"[^>]*><!--.*?"objectUrn":"urn:li:organization:(\d+)""#,
  )
});
static OWN_URN: LazyLock<Regex> =
  LazyLock::new(|| re(r#"data-semaphore-content-urn="urn:li:organization:(\d+)""#));
static SEE_JOBS: LazyLock<Regex> =
  LazyLock::new(|| re(r#"top-card-layout__cta[^"]*" href="[^"]*[?&]f_C=([0-9%Cc,]+)"#));
static ABOUT: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)data-test-id="about-us__description"[^>]*>(.*?)</p>"#));
static LINK: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<a[^>]*>(.*?)</a>"));
static FACT: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)data-test-id="about-us__(\w+)"[^>]*>\s*<dt[^>]*>.*?</dt>\s*<dd[^>]*>(.*?)</dd>"#)
});

/// "About us" keys, as `extra` names them.
const FACTS: &[(&str, &str)] = &[
  ("website", "website"),
  ("industry", "industry"),
  ("size", "company_size"),
  ("organizationType", "company_type"),
  ("foundedOn", "founded"),
  ("specialties", "specialties"),
];

/// What the company's jobs are searched by: the numbers of its "See
/// jobs" link (the company and pages of its own), else its own number.
pub fn job_ids(html: &str) -> Option<String> {
  SEE_JOBS
    .captures(html)
    .map(|c| c[1].replace("%2C", ",").replace("%2c", ","))
    .or_else(|| own_id(html))
}

/// The company's number.
pub fn own_id(html: &str) -> Option<String> {
  ORGANIZATION
    .captures(html)
    .or_else(|| OWN_URN.captures(html))
    .map(|c| c[1].to_owned())
}

/// A company's page as a user; `None` when it is not one. `name` is the
/// name it was asked by, for pages without a canonical link.
pub fn company(html: &str, name: &str) -> Option<User> {
  let title = capture(&NAME, html)?;
  let url = CANONICAL.captures(html).map(|c| attr(&c[1]));
  let handle = url
    .as_deref()
    .and_then(refs::company_name)
    .unwrap_or_else(|| name.to_owned());
  let headquarters = fact(html, "headquarters");
  let mut user = User {
    id: handle.clone(),
    name: title,
    url: Some(url.unwrap_or_else(|| refs::CompanyRef::Name(handle.clone()).url())),
    handle: Some(handle),
    avatar: IMAGE.captures(html).map(|c| attr(&c[1])),
    bio: capture(&ABOUT, html).or_else(|| capture(&TAGLINE, html)),
    location: headquarters.clone(),
    stats: UserStats {
      followers: FOLLOWERS.captures(html).and_then(|c| parse_count(&c[1])),
      ..UserStats::default()
    },
    ..User::default()
  };
  let extra = &mut user.extra;
  if let Some(id) = own_id(html) {
    extra.insert("company_id".into(), json!(id));
  }
  if let Some(ids) = job_ids(html) {
    extra.insert("jobs_company_ids".into(), json!(ids));
  }
  if let Some(tagline) = capture(&TAGLINE, html) {
    extra.insert("tagline".into(), json!(tagline));
  }
  let mut facts = Vec::new();
  for (key, name) in FACTS {
    let mut value = fact(html, key);
    if *key == "industry" && value.is_none() {
      value = capture(&HEADLINE, html);
    }
    if let Some(value) = value {
      facts.push((*name, Some(json!(value))));
      extra.insert((*name).into(), json!(value));
    }
  }
  facts.push(("headquarters", headquarters.map(Value::from)));
  user.raw = Some(read_parts(&facts));
  Some(user)
}

/// One "About us" fact by its key (`size`, `headquarters` …); a link's
/// text (the website, without the words for screen readers beside it).
fn fact(html: &str, key: &str) -> Option<String> {
  let value = FACT.captures_iter(html).find(|c| &c[1] == key)?[2].to_owned();
  let shown = LINK
    .captures(&value)
    .map_or(value.as_str(), |c| c.get(1).map_or("", |m| m.as_str()));
  Some(line(shown)).filter(|v| !v.is_empty())
}
