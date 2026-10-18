//! Proginn payloads → the shared model: a job (the records of the search
//! and of `getRecruit`) as a post with the shared job keys and its
//! company as the author, and the account of `getInfo`.
//!
//! Whoever posted a job (`ownerInfo`, the poster's `uid`, when they were
//! last online) and contact details (`emailShow`, phone numbers, e-mail)
//! are dropped, from `raw` too ([`sanitize`]).

use atp_core::jobs::{Period, Salary};
use atp_core::text::from_secs;
use atp_core::{Extra, Metrics, Post, User, Value, ValueExt, json};

use crate::api::WWW;
use crate::refs::job_url;

/// Keys about a job's poster or anyone's contact details, dropped
/// wherever they appear.
const DROPPED: &[&str] = &[
  "ownerInfo",
  "emailShow",
  "uid",
  "lineStatus",
  "aliveFormat",
  "replyStateFormat",
  "email",
  "mobile",
  "login_mobile",
  "phone",
  "wechat",
  "qq",
  "realname",
];

/// What the site writes where it has nothing to say.
const NOTHING: &str = "暂无";

/// The payload without the poster and contact details.
pub fn sanitize(v: &Value) -> Value {
  match v {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .filter(|(key, _)| !DROPPED.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), sanitize(value)))
        .collect(),
    ),
    Value::Array(items) => Value::Array(items.iter().map(sanitize).collect()),
    other => other.clone(),
  }
}

/// A trimmed, non-empty string that says something.
fn text(v: &Value, path: &str) -> Option<String> {
  v.str(path)
    .map(|s| s.trim().to_owned())
    .filter(|s| !s.is_empty() && s != NOTHING)
}

/// A job record (`hashId`, `recruitId`, `title`, `description` …).
pub fn job(v: &Value) -> Option<Post> {
  let id = v.first_str(&["recruitId", "id"])?;
  let url = v
    .str("hashId")
    .map(|hash| job_url(&hash))
    .or_else(|| v.str("link"));
  let mut post = Post {
    kind: "job".into(),
    title: text(v, "title"),
    text: v
      .str("description")
      .map(|t| description(&t))
      .filter(|t| !t.is_empty()),
    url,
    author: company(v.at("companyInfo")),
    created_at: v.i64("createdAt").and_then(from_secs),
    updated_at: v.i64("updatedAt").and_then(from_secs),
    metrics: metrics(v),
    tags: v
      .list("skills")
      .iter()
      .filter_map(|s| text(s, "name"))
      .collect(),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  let company = post.author.as_ref().map(|c| c.name.clone());
  fill(&mut post.extra, v, company);
  Some(post)
}

/// The description as written, with Unix line ends and without the
/// indentation the site puts after its 项目编号 line.
fn description(text: &str) -> String {
  let lines: Vec<&str> = text.lines().map(str::trim).collect();
  lines.join("\n").trim().to_owned()
}

/// Views and saves (job pages only) and applications.
fn metrics(v: &Value) -> Metrics {
  let mut metrics = Metrics {
    views: v.count("pv"),
    favorites: v.count("collectCount"),
    ..Metrics::default()
  };
  if let Some(n) = v.count("applicationCount") {
    metrics.other.insert("applications".into(), n);
  }
  metrics
}

/// The job keys: company, pay, city, remote, experience, the role and
/// its direction, the state and the deadline.
fn fill(extra: &mut Extra, v: &Value, company: Option<String>) {
  if let Some(company) = company {
    extra.insert("company".into(), json!(company));
  }
  if let Some(pay) = salary(v) {
    pay.fill(extra);
  }
  if let Some(city) = text(v, "cityName").or_else(|| text(v, "provinceName")) {
    extra.insert("city".into(), json!(city));
  }
  match v.i64("workType") {
    Some(1) => extra.insert("remote".into(), json!(true)),
    Some(2) => extra.insert("remote".into(), json!(false)),
    _ => None,
  };
  for (key, path) in [
    ("experience", "experienceName"),
    ("role", "occupationName"),
    ("direction", "directionName"),
    ("status", "statusName"),
  ] {
    if let Some(value) = text(v, path) {
      extra.insert(key.into(), json!(value));
    }
  }
  if let Some(deadline) = v.i64("endtime").and_then(day) {
    extra.insert("deadline".into(), json!(deadline));
  }
}

/// The pay as shown (`￥6-12K/月`), else from its figures (thousands of
/// yuan a month).
fn salary(v: &Value) -> Option<Salary> {
  let shown = text(v, "salaryName");
  let parsed = shown.as_deref().and_then(Salary::parse);
  if parsed
    .as_ref()
    .is_some_and(|pay| pay.min.is_some() || pay.max.is_some())
  {
    return parsed;
  }
  let figure = |path| v.f64(path).filter(|n| *n > 0.0).map(|n| n * 1000.0);
  let (min, max) = (figure("minSalary"), figure("maxSalary"));
  if min.is_none() && max.is_none() {
    return parsed;
  }
  let mut pay = Salary::new(min, max, Period::Month);
  pay.text = shown.unwrap_or_default();
  Some(pay)
}

/// A day in Beijing time, from Unix seconds.
fn day(secs: i64) -> Option<String> {
  let beijing = jiff::tz::TimeZone::fixed(jiff::tz::offset(8));
  from_secs(secs).map(|at| at.to_zoned(beijing).date().to_string())
}

/// The company a job is for (`companyInfo`): the name it goes by, its
/// logo and what it says about itself. Proginn has no company pages, so
/// the full name is its id.
pub fn company(v: &Value) -> Option<User> {
  let full = text(v, "name").or_else(|| text(v, "shortName"))?;
  let name = text(v, "shortName").unwrap_or_else(|| full.clone());
  let mut user = User {
    avatar: text(v, "logo"),
    bio: v.str("description").map(|t| description(&t)),
    ..User::default()
  };
  user.bio = user.bio.filter(|b| !b.is_empty() && b != NOTHING);
  for (key, value) in [
    ("full_name", Some(full.clone()).filter(|f| *f != name)),
    ("brief", text(v, "briefIntroduction")),
    ("website", text(v, "website")),
  ] {
    if let Some(value) = value {
      user.extra.insert(key.into(), json!(value));
    }
  }
  user.id = full;
  user.name = name;
  Some(user)
}

/// The logged-in account (`getInfo`).
pub fn account(v: &Value) -> Option<User> {
  let id = v.str("uid").filter(|id| id != "0")?;
  let mut user = User {
    url: Some(format!("{WWW}/wo/{id}")),
    name: text(v, "nickname").unwrap_or_else(|| id.clone()),
    avatar: text(v, "icon_url"),
    raw: Some(sanitize(v)),
    id,
    ..User::default()
  };
  if let Some(state) = v.i64("realname_re") {
    let verified = state == REAL_NAME_VERIFIED;
    user
      .extra
      .insert("real_name_verified".into(), json!(verified));
  }
  if let Some(vip) = v.i64("is_vip") {
    user.extra.insert("vip".into(), json!(vip != 0));
  }
  if let Some(until) = text(v, "vip_end_date") {
    user.extra.insert("vip_until".into(), json!(until));
  }
  Some(user)
}

/// `realname_re` of an account whose real name the site verified.
pub const REAL_NAME_VERIFIED: i64 = 2;
