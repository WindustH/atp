//! Alibaba's answers: the page's token, batches, a batch's filters and
//! jobs (search rows and job pages alike).

use atp_core::text::from_millis;
use atp_core::{Collection, Value, ValueExt, json};
use jiff::Timestamp;

use super::{Job, joined, names, text};
use crate::alibaba::{CAMPUS, TALENT};
use crate::jobs::Kind;

const COMPANY: &str = "阿里巴巴";

/// `__token__: "…"` of the page's `window.__sysconfig`.
pub fn token(html: &str) -> Option<String> {
  let rest = &html[html.find("__token__")? + "__token__".len()..];
  let rest = &rest[rest.find('"')? + 1..];
  let token = &rest[..rest.find('"')?];
  (!token.is_empty() && token.len() < 100).then(|| token.to_owned())
}

/// The batches of graduates and internships, in the site's order
/// (`sequence`); the special talent plans are left out.
pub fn batches(v: &Value) -> Vec<Collection> {
  let order: Vec<String> = names(v, "sequence", None);
  let groups = if order.is_empty() {
    vec!["graduate".to_owned(), "internship".to_owned()]
  } else {
    order
  };
  groups
    .iter()
    .filter(|g| matches!(g.as_str(), "graduate" | "internship"))
    .flat_map(|group| {
      v.list(group).iter().filter_map(move |b| {
        Some(Collection {
          id: b.str("id")?,
          kind: "batch".into(),
          name: b.str("name")?,
          description: b.str("remark"),
          extra: [("group".to_owned(), json!(group))].into_iter().collect(),
          ..Collection::default()
        })
      })
    })
    .collect()
}

/// One of a batch's filters (`workCity`, `category`), as codes.
pub fn conditions(v: &Value, kind: &str, name: &str) -> Vec<Collection> {
  let items = v
    .as_array()
    .map(Vec::as_slice)
    .unwrap_or_else(|| v.list("searchItems"));
  items
    .iter()
    .filter(|item| item.str("type").as_deref() == Some(kind))
    .flat_map(|item| item.list("items"))
    .filter_map(|c| {
      Some(Collection {
        id: c.str("value")?,
        kind: name.into(),
        name: c.str("label")?,
        ..Collection::default()
      })
    })
    .collect()
}

pub fn job_url(id: &str, social: bool) -> String {
  if social {
    format!("{TALENT}/off-campus/position-detail?positionId={id}")
  } else {
    format!("{CAMPUS}/campus/position/{id}")
  }
}

/// `degree`: bachelor → 本科 …
fn degree(code: &str) -> String {
  match code {
    "bachelor" => "本科",
    "master" => "硕士",
    "doctor" | "phd" => "博士",
    "college" | "junior_college" => "大专",
    other => other,
  }
  .to_owned()
}

/// `experience` `{from, to}` in years.
fn experience(v: &Value) -> Option<String> {
  match (v.u64("from"), v.u64("to")) {
    (Some(from), Some(to)) => Some(format!("{from}-{to}年")),
    (Some(from), None) => Some(format!("{from}年以上")),
    (None, Some(to)) => Some(format!("{to}年以内")),
    (None, None) => None,
  }
}

/// A day of milliseconds, as `2027-10-31`.
fn day(ms: Option<i64>) -> Option<String> {
  let ts: Timestamp = ms.and_then(from_millis)?;
  Some(
    ts.to_zoned(jiff::tz::TimeZone::fixed(jiff::tz::offset(8)))
      .date()
      .to_string(),
  )
}

/// A job, from the search or its page.
pub fn job(v: &Value, social: bool) -> Option<Job> {
  let id = v.str("id")?;
  let batch = v.str("batchName");
  let recruit = if social {
    Kind::Social
  } else if batch.as_deref().is_some_and(|b| b.contains("实习")) {
    Kind::Intern
  } else {
    Kind::Campus
  };
  let categories = names(v, "categories", None);
  let category = v
    .str("categoryName")
    .or_else(|| (!categories.is_empty()).then(|| categories.join("、")));
  let mut job = Job {
    url: job_url(&id, social),
    title: v.str("name"),
    text: joined(&[text(v, "description"), text(v, "requirement")]),
    company: COMPANY.into(),
    site: if social { TALENT } else { CAMPUS }.into(),
    cities: names(v, "workLocations", None),
    recruit: Some(recruit),
    category,
    department: v.str("department"),
    batch,
    experience: experience(v.at("experience")),
    degree: v.str("degree").as_deref().map(degree),
    code: v.str("code"),
    created_at: v.i64("publishTime").and_then(from_millis),
    updated_at: v.i64("modifyTime").and_then(from_millis),
    tags: names(v, "circleNames", None),
    raw: v.clone(),
    id,
    ..Job::default()
  };
  let graduation = v.at("graduationTime");
  if let (Some(from), Some(to)) = (day(graduation.i64("from")), day(graduation.i64("to"))) {
    job
      .more
      .push(("graduation", json!(format!("{from} – {to}"))));
  }
  let interviews = names(v, "interviewLocations", None);
  if !interviews.is_empty() {
    job.more.push(("interview_cities", json!(interviews)));
  }
  Some(job)
}
