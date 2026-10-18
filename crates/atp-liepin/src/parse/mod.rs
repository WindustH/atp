//! Liepin payloads → the shared model: job cards (search, company jobs,
//! saved jobs, applications, invitations) as posts with the shared job
//! keys in `extra`, companies as users, recruiter chats as collections
//! and their messages as posts; job and company pages through [`page`].
//!
//! Contact details (phone numbers, WeChat, e-mail) are dropped wherever
//! a payload carries them, `raw` included ([`sanitize`]), and exchanged
//! contact cards in chats are never shown.

mod chats;
mod page;
mod salary;

pub use chats::{chat, message};
pub use page::{company, config, job};

use atp_core::model::Extra;
use atp_core::text::{from_beijing, from_unix, parse_time};
use atp_core::{Post, User, Value, ValueExt, json};
use jiff::Timestamp;

use crate::refs;

/// Parts of keys that hold contact details, dropped wherever they appear.
const CONTACT_PARTS: &[&str] = &["phone", "mobile", "wechat", "weixin", "email", "telnum"];
const CONTACT_KEYS: &[&str] = &["tel", "qq", "wx", "wxid"];

fn contact_key(key: &str) -> bool {
  let key = key.to_lowercase();
  CONTACT_KEYS.contains(&key.as_str()) || CONTACT_PARTS.iter().any(|part| key.contains(part))
}

/// The payload without contact details.
pub fn sanitize(v: &Value) -> Value {
  match v {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .filter(|(key, _)| !contact_key(key))
        .map(|(key, value)| (key.clone(), sanitize(value)))
        .collect(),
    ),
    Value::Array(items) => Value::Array(items.iter().map(sanitize).collect()),
    other => other.clone(),
  }
}

/// A picture's link: the file names cards give live on the image host.
pub fn image(name: &str) -> Option<String> {
  let name = name.trim();
  if name.is_empty() {
    None
  } else if name.starts_with("http") {
    Some(name.to_owned())
  } else if let Some(rest) = name.strip_prefix("//") {
    Some(format!("https://{rest}"))
  } else {
    Some(format!("https://image0.lietou-static.com/big/{name}"))
  }
}

/// A time as Liepin writes it: `20261018000352` (Beijing time), Unix
/// seconds or milliseconds, `2026-10-18 09:37:00` or RFC 3339.
pub fn time(v: &Value) -> Option<Timestamp> {
  if let Some(n) = v.as_i64() {
    return from_unix(n);
  }
  let text = v.as_str()?.trim();
  if text.len() == 14 && text.bytes().all(|b| b.is_ascii_digit()) {
    let local = format!(
      "{}-{}-{} {}:{}:{}",
      &text[..4],
      &text[4..6],
      &text[6..8],
      &text[8..10],
      &text[10..12],
      &text[12..]
    );
    return from_beijing(&local);
  }
  if let Ok(n) = text.parse::<i64>() {
    return from_unix(n);
  }
  parse_time(text).or_else(|| from_beijing(&text.replace('T', " ")))
}

/// A link without its query (the tracking parameters cards add).
fn clean_link(link: &str) -> String {
  link.split(['?', '#']).next().unwrap_or(link).to_owned()
}

/// The company of a card, when it names one.
fn card_company(comp: &Value) -> Option<User> {
  let id = comp.str("compId").filter(|id| !id.is_empty());
  let name = comp.str("compName").filter(|n| !n.is_empty());
  if id.is_none() && name.is_none() {
    return None;
  }
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    name: name.clone().or_else(|| id.clone()).unwrap_or_default(),
    avatar: comp.str("compLogo").as_deref().and_then(image),
    id: id.unwrap_or_default(),
    ..User::default()
  };
  for (key, path) in [
    ("industry", "compIndustry"),
    ("size", "compScale"),
    ("stage", "compStage"),
  ] {
    if let Some(value) = comp.str(path).filter(|v| !v.is_empty()) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}

/// Labels: strings, or objects naming one.
fn labels(list: &[Value]) -> Vec<String> {
  list
    .iter()
    .filter_map(|l| {
      l.as_str()
        .map(str::to_owned)
        .or_else(|| l.first_str(&["name", "text", "label"]))
    })
    .filter(|l| !l.trim().is_empty())
    .collect()
}

/// `job_type` of a campus job kind (`实习`, `校招`).
fn job_type(kind: &str) -> Option<&'static str> {
  match kind {
    k if k.contains("实习") => Some("intern"),
    k if k.contains("兼职") => Some("part-time"),
    k if k.contains("校招") || k.contains("应届") || k.contains("全职") => Some("full-time"),
    _ => None,
  }
}

/// Who posted a job, by its kind: 1 a headhunter (猎头职位), else the
/// company itself (企业职位).
pub fn posted_by(kind: &str) -> &'static str {
  if kind == "1" { "headhunter" } else { "company" }
}

/// A job card: `{job, comp, recruiter}`.
pub fn card(v: &Value) -> Post {
  let job = v.at("job");
  let link = job.str("link").map(|l| clean_link(&l));
  let id = link
    .as_deref()
    .and_then(refs::job_id)
    .or_else(|| job.str("jobId"))
    .unwrap_or_default();
  let company = card_company(v.at("comp"));
  let mut post = Post {
    kind: "job".into(),
    title: job.str("title"),
    url: link,
    updated_at: time(job.at("refreshTime")),
    tags: labels(job.list("labels")),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = &company {
    extra.insert("company".into(), json!(company.name));
    for key in ["industry", "size", "stage"] {
      if let Some(value) = company.extra.get(key) {
        extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  job_fields(extra, job);
  recruiter(extra, v.at("recruiter"), "recruiterName", "recruiterTitle");
  post.author = company;
  post
}

/// The job keys of a card's `job`: pay, city, experience, degree, kind.
fn job_fields(extra: &mut Extra, job: &Value) {
  if let Some(pay) = job.str("salary").as_deref().and_then(salary::parse) {
    pay.fill(extra);
  }
  for (key, path) in [
    ("city", "dq"),
    ("experience", "requireWorkYears"),
    ("degree", "requireEduLevel"),
    ("job_id", "jobId"),
  ] {
    if let Some(value) = job.str(path) {
      extra.insert(key.into(), json!(value));
    }
  }
  if let Some(kind) = job.str("jobKind") {
    extra.insert("job_kind".into(), json!(kind));
    extra.insert("posted_by".into(), json!(posted_by(&kind)));
  }
  if let Some(kind) = job.str("campusJobKind").as_deref().and_then(job_type) {
    extra.insert("job_type".into(), json!(kind));
  }
  for (key, path) in [("top", "topJob"), ("closed", "finished")] {
    if job.bool(path) == Some(true) {
      extra.insert(key.into(), json!(true));
    }
  }
}

/// The recruiter a card or page shows: name and title only.
fn recruiter(extra: &mut Extra, v: &Value, name: &str, title: &str) {
  let name = v.str(name).filter(|n| !n.is_empty());
  if name.is_none() {
    return;
  }
  extra.insert(
    "recruiter".into(),
    json!({"name": name, "title": v.str(title).filter(|t| !t.is_empty())}),
  );
}

/// States of an application, as the site names them.
fn application_status(code: &str) -> Option<(&'static str, &'static str)> {
  Some(match code {
    "-1" => ("expired", "反馈超时"),
    "0" => ("applied", "已投递"),
    "1" => ("viewed", "已查看"),
    "2" => ("rejected", "暂不合适"),
    "3" => ("interview", "面试邀约"),
    _ => return None,
  })
}

/// One of your applications (`apply-list`): the job card with the
/// application's state.
pub fn application(v: &Value) -> Post {
  let mut post = card(v);
  post.kind = "application".into();
  let info = v.at("applyInfo");
  let steps = info.list("applyStatusList");
  post.created_at = steps.first().and_then(|s| time(s.at("createtime")));
  if let Some(at) = time(info.at("statusTime")) {
    post.updated_at = Some(at);
  }
  let extra = &mut post.extra;
  if let Some(id) = info.str("applyId") {
    extra.insert("application_id".into(), json!(id));
  }
  if let Some((status, label)) = info
    .str("applyStatus")
    .as_deref()
    .and_then(application_status)
  {
    extra.insert("status".into(), json!(status));
    extra.insert("status_label".into(), json!(label));
  }
  let history: Vec<Value> = steps
    .iter()
    .filter(|s| s.bool("hasFinished") != Some(false))
    .map(|s| json!({"status": s.str("statusInfo"), "at": time(s.at("createtime"))}))
    .collect();
  if !history.is_empty() {
    extra.insert("history".into(), json!(history));
  }
  post
}

/// An invitation to apply (邀请应聘) a recruiter sent you.
pub fn invitation(v: &Value) -> Post {
  let link = v.str("pcJobUrl").map(|l| clean_link(&l));
  let mut post = Post {
    id: v
      .str("id")
      .or_else(|| link.as_deref().and_then(refs::job_id))
      .unwrap_or_default(),
    kind: "invitation".into(),
    title: v.str("title"),
    text: v.str("inviteDesc").filter(|t| !t.is_empty()),
    url: link,
    tags: labels(v.list("jobCardTags")),
    raw: Some(sanitize(v)),
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = v.str("company").filter(|c| !c.is_empty()) {
    extra.insert("company".into(), json!(company));
  }
  if let Some(pay) = v.str("salary").as_deref().and_then(salary::parse) {
    pay.fill(extra);
  }
  if let Some(id) = v.str("jobId") {
    extra.insert("job_id".into(), json!(id));
  }
  if let Some(kind) = v.str("jobKind") {
    extra.insert("posted_by".into(), json!(posted_by(&kind)));
  }
  extra.insert("accepted".into(), json!(v.bool("apply") == Some(true)));
  recruiter(extra, v, "recruiterName", "recruiterTitle");
  post
}
