//! BOSS Zhipin payloads → the shared model: job cards (the search, your
//! saved jobs, chatted jobs and applications) as posts with the shared
//! job keys in `extra`, companies as users, interviews as posts, chats
//! and their messages ([`chats`]); job and company pages ([`pages`]).
//!
//! Recruiters (their names, titles, pictures, account ids, whether they
//! are online) and contact details are dropped from every payload, `raw`
//! included ([`sanitize`]); a chat is named by its company and job. The
//! pay's figures, where a page writes them with the site's font, are
//! decoded ([`crate::font`]).

mod chats;
mod pages;

pub use chats::{chat, message};
pub use pages::{company_page, job_page};

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::text::from_millis;
use atp_core::{Post, User, Value, ValueExt, json};
use jiff::Timestamp;

use crate::{font, refs};

/// Parts of keys about recruiters or contacts, dropped wherever they
/// appear (`bossName`, `encryptBossId`, `bossAvatar`, `phone` …).
const DROP_PARTS: &[&str] = &[
  "boss",
  "avatar",
  "goldhunter",
  "phone",
  "mobile",
  "wechat",
  "weixin",
  "email",
];

fn dropped(key: &str) -> bool {
  let lower = key.to_lowercase();
  DROP_PARTS.iter().any(|part| lower.contains(part))
}

/// The payload without recruiters and contact details.
pub fn sanitize(v: &Value) -> Value {
  match v {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .filter(|(key, _)| !dropped(key))
        .map(|(key, value)| (key.clone(), sanitize(value)))
        .collect(),
    ),
    Value::Array(items) => Value::Array(items.iter().map(sanitize).collect()),
    other => other.clone(),
  }
}

/// A text field, trimmed, when not empty.
pub fn text(v: &Value, path: &str) -> Option<String> {
  v.str(path)
    .map(|s| s.trim().to_owned())
    .filter(|s| !s.is_empty())
}

/// A time in milliseconds, as the site's lists give them.
pub fn time(v: &Value, path: &str) -> Option<Timestamp> {
  v.i64(path).filter(|ms| *ms > 0).and_then(from_millis)
}

/// The strings of a list, trimmed and without repeats.
fn strings(lists: &[&[Value]]) -> Vec<String> {
  let mut seen = std::collections::HashSet::new();
  lists
    .iter()
    .flat_map(|list| list.iter())
    .filter_map(|v| v.as_str())
    .map(|s| s.trim().to_owned())
    .filter(|s| !s.is_empty() && seen.insert(s.clone()))
    .collect()
}

/// The pay as shown (its hidden digits decoded) into the job keys.
pub fn salary(extra: &mut Extra, shown: &str) {
  let shown = font::decode(shown.trim());
  if shown.is_empty() {
    return;
  }
  match Salary::parse(&shown) {
    Some(pay) => pay.fill(extra),
    None => {
      extra.insert("salary".into(), json!(shown));
    }
  }
}

/// A company of a card: its id, name, logo, page, industry, funding
/// stage and size.
pub fn company(v: &Value) -> Option<User> {
  let name = text(v, "brandName")?;
  let id = text(v, "encryptBrandId");
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    avatar: text(v, "brandLogo").filter(|l| l.starts_with("http")),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  };
  for (key, path) in [
    ("industry", "brandIndustry"),
    ("stage", "brandStageName"),
    ("size", "brandScaleName"),
  ] {
    if let Some(value) = text(v, path) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}

/// The company's name and facts as job keys.
pub fn company_keys(extra: &mut Extra, company: &User) {
  extra.insert("company".into(), json!(company.name));
  for key in ["industry", "stage", "size"] {
    if let Some(value) = company.extra.get(key) {
      extra.insert(format!("company_{key}"), value.clone());
    }
  }
}

/// A job card of the search and of the account's lists.
pub fn card(v: &Value) -> Option<Post> {
  let id = text(v, "encryptJobId")?;
  let author = company(v);
  let link = refs::job_link(
    &id,
    text(v, "securityId").as_deref(),
    text(v, "lid").as_deref(),
  );
  let mut post = Post {
    url: Some(link),
    kind: "job".into(),
    title: text(v, "jobName"),
    tags: strings(&[v.list("skills"), v.list("welfareList")]),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = &author {
    company_keys(extra, company);
  }
  salary(extra, &v.str("salaryDesc").unwrap_or_default());
  for (key, path) in [
    ("city", "cityName"),
    ("district", "areaDistrict"),
    ("business_district", "businessDistrict"),
    ("experience", "jobExperience"),
    ("degree", "jobDegree"),
    ("days_per_week", "daysPerWeekDesc"),
    ("months", "leastMonthDesc"),
  ] {
    if let Some(value) = text(v, path) {
      extra.insert(key.into(), json!(value));
    }
  }
  post.author = author;
  Some(post)
}

/// A company the account follows (`brand/focus/querylist`, whose
/// `brandId` is the company's id as its page's link has it).
pub fn followed(v: &Value) -> Option<User> {
  let id = text(v, "brandId").or_else(|| text(v, "encryptBrandId"))?;
  let mut user = User {
    url: Some(refs::company_url(&id)),
    name: text(v, "brandName").unwrap_or_else(|| id.clone()),
    avatar: text(v, "brandLogo").filter(|l| l.starts_with("http")),
    followed: Some(true),
    id,
    raw: Some(sanitize(v)),
    ..User::default()
  };
  for (key, path) in [
    ("industry", "industryName"),
    ("stage", "stageName"),
    ("size", "scaleName"),
  ] {
    if let Some(value) = text(v, path) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  let benefits = strings(&[v.list("welfareList")]);
  if !benefits.is_empty() {
    user.extra.insert("benefits".into(), json!(benefits));
  }
  if v.bool("rejected") == Some(true) {
    user.extra.insert("closed".into(), json!(true));
  }
  Some(user)
}

/// An interview you were invited to (`interview/list`): the job, the
/// company, when and how.
pub fn interview(v: &Value) -> Post {
  let job = text(v, "encryptJobId");
  let mut post = Post {
    id: text(v, "encryptInterviewId")
      .or_else(|| text(v, "interviewId"))
      .or_else(|| job.clone())
      .unwrap_or_default(),
    kind: "interview".into(),
    title: text(v, "jobName").or_else(|| text(v, "positionName")),
    url: job.as_deref().map(|id| {
      refs::job_link(
        id,
        text(v, "securityId").as_deref(),
        text(v, "lid").as_deref(),
      )
    }),
    created_at: time(v, "addTime"),
    raw: Some(sanitize(v)),
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = text(v, "brandName").or_else(|| text(v, "companyName")) {
    extra.insert("company".into(), json!(company));
  }
  salary(extra, &v.str("salaryDesc").unwrap_or_default());
  if let Some(at) = time(v, "appointmentDate").or_else(|| time(v, "appointmentTime")) {
    extra.insert("interview_at".into(), json!(at));
  }
  for (key, path) in [
    ("status", "statusDesc"),
    ("address", "address"),
    ("city", "cityName"),
  ] {
    if let Some(value) = text(v, path) {
      extra.insert(key.into(), json!(value));
    }
  }
  if v.bool("videoInterview") == Some(true) {
    extra.insert("video".into(), json!(true));
  }
  post
}
