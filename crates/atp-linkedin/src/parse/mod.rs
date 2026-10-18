//! LinkedIn's HTML → the shared model: the cards of the job search
//! ([`cards`]), a job's page ([`job`]) and a company's page ([`company`]).
//! What they share lives here: text of elements, the company a job
//! names, its pay, `remote` from its words, and times as LinkedIn writes
//! them (a date, or `2 weeks ago`).
//!
//! Only the parts named here are read: people on these pages (the hiring
//! team, the poster, employees, "who you know") are never looked at, and
//! `raw` holds the parts read, not the page.

pub mod cards;
pub mod company;
pub mod job;

use std::sync::LazyLock;

use atp_core::jobs::{Period, Salary};
use atp_core::text::{date_only, html_to_text};
use atp_core::{User, Value, json};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use regex::Regex;

use crate::refs;

pub fn re(pattern: &str) -> Regex {
  Regex::new(pattern).expect("valid pattern")
}

/// A tag or a comment.
static TAG: LazyLock<Regex> = LazyLock::new(|| re(r"<[^>]*>"));
/// `N units ago`, as LinkedIn dates job pages (`Reposted 2 weeks ago`).
static AGO: LazyLock<Regex> =
  LazyLock::new(|| re(r"(?i)(\d+)\s+(second|minute|hour|day|week|month|year)s?\s+ago"));
/// A period named in pay (`/yr`, `per hour`).
static PER: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r"(?i)/\s*(?:hr|hour|day|wk|week|mo|month|yr|year)\b|\bper\s+(?:hour|day|week|month|year|annum)\b|\b(?:hourly|daily|weekly|monthly|yearly|annually)\b|\ban?\s+(?:hour|day|week|month|year)\b",
  )
});
/// Lines of a description that speak of pay.
static PAY_WORDS: LazyLock<Regex> =
  LazyLock::new(|| re(r"(?i)\b(?:salary|salaries|pay|compensation|wage|wages|hourly rate|OTE)\b"));
/// Words of a job's title or place that say it is remote.
static REMOTE: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r"(?i)[(\[]\s*(?:fully\s+|100%\s+)?remote\b|\bremote\s*[)\]]|[-–|,]\s*remote\s*$|^remote\s*[-–|:]",
  )
});

/// The first capture of `re` in `html`, as one line of text.
pub fn capture(re: &Regex, html: &str) -> Option<String> {
  re.captures(html)
    .map(|m| line(&m[1]))
    .filter(|s| !s.is_empty())
}

/// HTML (a title, a name, a place) as one line of text: the text of its
/// elements, entities decoded.
pub fn line(html: &str) -> String {
  html_to_text(&TAG.replace_all(html, " "))
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
}

/// The first line of text of some HTML (`Be an early applicant` of a
/// note going on with `+3 benefits`).
pub fn first_line(html: &str) -> String {
  let text = TAG.replace_all(html, "");
  line(
    text
      .lines()
      .find(|l| !l.trim().is_empty())
      .unwrap_or_default(),
  )
}

/// A link as written in an attribute (`&amp;` and all).
pub fn attr(value: &str) -> String {
  value.replace("&amp;", "&").trim().to_owned()
}

/// The company a job names: its name, and a `User` with its page when it
/// has one (`https://www.linkedin.com/company/<name>`), its id being the
/// name in that link.
pub fn employer(name: &str, link: Option<&str>, logo: Option<String>) -> Option<User> {
  let handle = link.and_then(refs::company_name)?;
  Some(User {
    id: handle.clone(),
    name: name.to_owned(),
    url: Some(refs::CompanyRef::Name(handle.clone()).url()),
    handle: Some(handle),
    avatar: logo,
    ..User::default()
  })
}

/// LinkedIn's base pay (`$117,000.00/yr - $234,000.00/yr`, `$25.00/hr`,
/// or without a period, `$120,000.00 - $150,000.00`). `¥` is the yen in
/// LinkedIn's English pages, the yuan being `CN¥`. Pay without a period
/// is taken by its size: below 300 an hour, from 20,000 a year, a month
/// between.
pub fn pay(shown: &str) -> Option<Salary> {
  let shown = line(shown);
  let text = if shown.contains("CN¥") {
    shown.clone()
  } else {
    shown.replace('¥', "JPY ")
  };
  let mut salary = Salary::parse(&text)?;
  salary.text = shown.clone();
  let largest = salary.max.or(salary.min)?;
  if !PER.is_match(&shown) {
    salary.period = if largest < 300.0 {
      Period::Hour
    } else if largest >= 20_000.0 {
      Period::Year
    } else {
      Period::Month
    };
  }
  Some(salary)
}

/// Pay a job's title names (`Engineer ($185K - $300K)`), else the first
/// range of pay in a line of its description that speaks of pay (`The
/// base salary range is 184,000 USD - 287,500 USD`); a single figure in a
/// description is not taken for the pay (bonuses, funding and the like).
pub fn pay_in(title: &str, description: Option<&str>) -> Option<Salary> {
  if let Some(salary) = Salary::in_text(title) {
    return Some(salary);
  }
  description?
    .lines()
    .filter(|line| PAY_WORDS.is_match(line))
    .filter_map(Salary::in_text)
    .find(|s| matches!((s.min, s.max), (Some(min), Some(max)) if min < max))
}

/// Whether a job's title or place says it is remote (when they do not,
/// the job may still be).
pub fn remote(title: &str, place: Option<&str>) -> bool {
  let place = place.unwrap_or_default().to_lowercase();
  place.contains("remote") || REMOTE.is_match(title)
}

/// A date as cards write it (`2026-10-07`), without a time of day.
pub fn day(text: &str) -> Option<Timestamp> {
  date_only(text.trim().get(..10)?.parse().ok()?)
}

/// `15 hours ago` as a time; `2 weeks ago` and longer as the day it was.
pub fn ago(text: &str) -> Option<Timestamp> {
  let caps = AGO.captures(text)?;
  let n: i64 = caps[1].parse().ok()?;
  let now = Timestamp::now();
  let unit = caps[2].to_lowercase();
  let seconds = match unit.as_str() {
    "second" => 1,
    "minute" => 60,
    "hour" => 3600,
    _ => 0,
  };
  if seconds > 0 {
    let then = now.as_second().checked_sub(n.checked_mul(seconds)?)?;
    return Timestamp::from_second(then).ok();
  }
  let span = match unit.as_str() {
    "day" => Span::new().try_days(n),
    "week" => Span::new().try_weeks(n),
    "month" => Span::new().try_months(n),
    _ => Span::new().try_years(n),
  }
  .ok()?;
  let today = now.to_zoned(TimeZone::UTC).date();
  date_only(today.checked_sub(span).ok()?)
}

/// The parts of a page that were read, as `raw`: the ones found.
pub fn read_parts(parts: &[(&str, Option<Value>)]) -> Value {
  let map = parts
    .iter()
    .filter_map(|(key, value)| Some(((*key).to_owned(), value.clone()?)))
    .collect();
  Value::Object(map)
}

/// A string as a JSON value, `None` staying out.
pub fn text_value(text: &Option<String>) -> Option<Value> {
  text.as_ref().map(|t| json!(t))
}
