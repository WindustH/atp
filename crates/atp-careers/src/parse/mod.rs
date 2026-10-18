//! Career sites' payloads → the shared model. Each backend reads its own
//! answers into a [`Job`] (one module each); [`Job::post`] writes it as a
//! post with the shared job keys in `extra`.
//!
//! Recruiters and other staff are dropped from every payload, `raw`
//! included ([`sanitize`]): their names, accounts, contact details and
//! QR codes.

pub mod alibaba;
pub mod beisen;
pub mod feishu;
pub mod kuaishou;
pub mod meituan;
pub mod moka;
pub mod tencent;

use atp_core::jobs::Salary;
use atp_core::text::{date_only, from_beijing, html_to_text};
use atp_core::{Post, User, Value, ValueExt, json};
use jiff::Timestamp;

use crate::jobs::Kind;

/// Parts of keys (lowercase, without `_`) that hold people or how to
/// reach them.
const PERSONAL: &[&str] = &[
  "phone",
  "mobile",
  "email",
  "wechat",
  "weixin",
  "qrcode",
  "contact",
  "recruiter",
  "leader",
  "observer",
  "interviewer",
  "creator",
  "crator",
  "incharge",
  "audituser",
  "auditstartuser",
  "closeuser",
  "staff",
  "publisher",
  "hrbp",
  "recommender",
];

fn personal(key: &str) -> bool {
  let key = key.to_lowercase().replace('_', "");
  PERSONAL.iter().any(|part| key.contains(part))
}

/// The payload without the people behind the job.
pub fn sanitize(v: &Value) -> Value {
  match v {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .filter(|(key, _)| !personal(key))
        .map(|(key, value)| (key.clone(), sanitize(value)))
        .collect(),
    ),
    Value::Array(items) => Value::Array(items.iter().map(sanitize).collect()),
    other => other.clone(),
  }
}

/// Text as a site writes it, which may be HTML; `None` when empty.
pub fn text(v: &Value, path: &str) -> Option<String> {
  let raw = v.str(path)?;
  let text = if raw.contains('<') {
    html_to_text(&raw)
  } else {
    raw.replace("\r\n", "\n").trim().to_owned()
  };
  (!text.is_empty()).then_some(text)
}

/// The parts of a job's text that are there, a blank line apart.
pub fn joined(parts: &[Option<String>]) -> Option<String> {
  let parts: Vec<&str> = parts.iter().flatten().map(String::as_str).collect();
  (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Strings of a list, or of objects in it by `key`.
pub fn names(v: &Value, path: &str, key: Option<&str>) -> Vec<String> {
  v.list(path)
    .iter()
    .filter_map(|item| match key {
      Some(key) => item.str(key),
      None => item.as_str().map(str::to_owned),
    })
    .map(|s| s.trim().to_owned())
    .filter(|s| !s.is_empty())
    .collect()
}

/// What a site tells about one job, in the shared shape.
#[derive(Debug, Default)]
pub struct Job {
  pub id: String,
  pub url: String,
  pub title: Option<String>,
  pub text: Option<String>,
  pub company: String,
  /// The site's page, the company's link.
  pub site: String,
  pub cities: Vec<String>,
  pub recruit: Option<Kind>,
  /// `job_type` when the site names it (实习, 全职, 兼职); else by `recruit`.
  pub job_type: Option<&'static str>,
  pub category: Option<String>,
  pub department: Option<String>,
  pub batch: Option<String>,
  pub experience: Option<String>,
  pub degree: Option<String>,
  pub deadline: Option<String>,
  pub address: Option<String>,
  pub headcount: Option<u64>,
  pub code: Option<String>,
  pub salary: Option<Salary>,
  pub created_at: Option<Timestamp>,
  pub updated_at: Option<Timestamp>,
  pub tags: Vec<String>,
  /// A site's own keys.
  pub more: Vec<(&'static str, Value)>,
  pub raw: Value,
}

impl Job {
  /// The job as a post; a job page (`detail`) adds where to apply.
  pub fn post(self, detail: bool) -> Post {
    let mut post = Post {
      id: self.id,
      url: Some(self.url.clone()),
      kind: "job".into(),
      title: self.title,
      text: self.text,
      author: Some(User {
        id: self.site.clone(),
        name: self.company.clone(),
        url: Some(self.site),
        ..User::default()
      }),
      created_at: self.created_at,
      updated_at: self.updated_at,
      tags: self.tags,
      raw: Some(sanitize(&self.raw)),
      ..Post::default()
    };
    let extra = &mut post.extra;
    extra.insert("company".into(), json!(self.company));
    if !self.cities.is_empty() {
      extra.insert("city".into(), json!(self.cities.join("/")));
    }
    let job_type = self.job_type.or(self.recruit.map(Kind::job_type));
    if let Some(job_type) = job_type {
      extra.insert("job_type".into(), json!(job_type));
    }
    if let Some(kind) = self.recruit {
      extra.insert("recruit_type".into(), json!(kind.name()));
    }
    for (key, value) in [
      ("category", self.category),
      ("department", self.department),
      ("batch", self.batch),
      ("experience", self.experience),
      ("degree", self.degree),
      ("deadline", self.deadline),
      ("address", self.address),
      ("job_code", self.code),
    ] {
      if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
        extra.insert(key.into(), json!(value.trim()));
      }
    }
    if let Some(n) = self.headcount.filter(|n| *n > 0) {
      extra.insert("headcount".into(), json!(n));
    }
    if let Some(salary) = &self.salary {
      salary.fill(extra);
    }
    for (key, value) in self.more {
      extra.insert(key.into(), value);
    }
    if detail {
      extra.insert("apply_url".into(), json!(self.url));
    }
    post
  }
}

/// A day as sites write it (`2026年10月15日`, `2026-10-15`), shown
/// without a time.
pub fn day(text: &str) -> Option<Timestamp> {
  let text = text.trim().replace(['年', '月'], "-").replace('日', "");
  let date: jiff::civil::Date = text.get(..10).unwrap_or(&text).parse().ok()?;
  date_only(date)
}

/// A local time in Beijing as sites write it (`2026-09-04T16:46:44.58`,
/// `2026-09-14 13:09:38`).
pub fn local(text: &str) -> Option<Timestamp> {
  let text = text.trim().replace('T', " ");
  from_beijing(text.get(..19).unwrap_or(&text))
}

/// `job_type` of the words sites use (全职, 实习, 兼职).
pub fn job_type(words: &str) -> Option<&'static str> {
  match words.trim() {
    w if w.contains("实习") || w.eq_ignore_ascii_case("intern") => Some("intern"),
    w if w.contains("兼职") || w.eq_ignore_ascii_case("parttime") => Some("part-time"),
    w if w.contains("全职") || w.contains("正式") || w.eq_ignore_ascii_case("fulltime") => {
      Some("full-time")
    }
    _ => None,
  }
}
