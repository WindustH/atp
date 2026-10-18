//! Entries of the 校招 schedule (校招日程, `school-schedule/list-card`): a
//! company's campus hiring for one batch (27届秋招, 日常实习 …), with when
//! applications open and close, the cities and careers, the referral code
//! (内推码) and the company's own application page (网申).

use atp_core::{Post, User, Value, ValueExt, json};

use super::{day, millis, sanitize};
use crate::refs;

fn names(v: &Value, path: &str) -> Vec<String> {
  v.list(path)
    .iter()
    .filter_map(|s| s.as_str().map(str::to_owned))
    .filter(|s| !s.trim().is_empty())
    .collect()
}

/// What the entry says, a line each: applications, referral code, the
/// application page, cities, careers, the source and the company's blurb.
fn summary(v: &Value) -> Option<String> {
  let mut lines = Vec::new();
  match (day(v, "wangshenBeginDate"), day(v, "wangshenEndDate")) {
    (Some(from), Some(to)) => lines.push(format!("网申 {from} – {to}")),
    (Some(from), None) => lines.push(format!("网申 {from} 起")),
    (None, Some(to)) => lines.push(format!("网申截止 {to}")),
    (None, None) => lines.extend(v.str("wangshenTime").map(|t| format!("网申 {t}"))),
  }
  if let Some(code) = v.str("referralCode").filter(|c| !c.trim().is_empty()) {
    lines.push(format!("内推码 {code}"));
  }
  if let Some(link) = v.str("customWangshenLink") {
    lines.push(format!("网申链接 {link}"));
  }
  for (label, path) in [("城市", "cityList"), ("岗位", "careerNameList")] {
    let list = names(v, path);
    if !list.is_empty() {
      lines.push(format!("{label} {}", list.join("、")));
    }
  }
  if let Some(source) = v.str("sourceInformation") {
    lines.push(format!("来源 {source}"));
  }
  if let Some(blurb) = v.str("companyEvaluation") {
    lines.push(String::new());
    lines.push(blurb);
  }
  (!lines.is_empty()).then(|| lines.join("\n"))
}

/// A schedule entry: the company, the batch, when applications close
/// (`deadline`), the referral code (`referral_code`) and the application
/// page (`apply_url`); its id is the company's, which
/// `schedule-subscribe` takes.
pub fn schedule(v: &Value) -> Option<Post> {
  let id = v.str("companyId").filter(|id| id != "0")?;
  let name = v.str("name")?;
  let batch = v.str("batchName");
  let code = v.str("referralCode").filter(|c| !c.trim().is_empty());
  let mut title = batch
    .as_ref()
    .map_or(name.clone(), |b| format!("{name} {b}"));
  if let Some(code) = &code {
    title.push_str(&format!(" · 内推码 {code}"));
  }
  let mut post = Post {
    url: Some(refs::schedule_url(&id)),
    kind: "schedule".into(),
    title: Some(title),
    text: summary(v),
    author: Some(User {
      id: id.clone(),
      name: name.clone(),
      url: Some(refs::company_url(&id)),
      avatar: v.str("homeLogo"),
      ..User::default()
    }),
    updated_at: millis(v, "wangshenUpdateTime").or_else(|| millis(v, "updateTime")),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  fill(&mut post, v, name, batch, code);
  Some(post)
}

fn fill(post: &mut Post, v: &Value, name: String, batch: Option<String>, code: Option<String>) {
  let extra = &mut post.extra;
  extra.insert("company".into(), json!(name));
  let cities = names(v, "cityList");
  if !cities.is_empty() {
    extra.insert("city".into(), json!(cities.join("/")));
  }
  for (key, value) in [
    ("batch", batch),
    ("referral_code", code),
    ("apply_url", v.str("customWangshenLink")),
    ("source", v.str("sourceInformation")),
    ("opens", day(v, "wangshenBeginDate")),
    ("deadline", day(v, "wangshenEndDate")),
    ("supplement_opens", day(v, "supplementBeginDate")),
    ("supplement_deadline", day(v, "supplementEndDate")),
  ] {
    if let Some(value) = value {
      extra.insert(key.into(), json!(value));
    }
  }
  for (key, path) in [
    ("careers", "careerNameList"),
    ("industries", "industryList"),
  ] {
    let list = names(v, path);
    if !list.is_empty() {
      extra.insert(key.into(), json!(list));
    }
  }
  for (key, path) in [("supplement", "supplementHire"), ("subscribed", "followed")] {
    if v.bool(path) == Some(true) {
      extra.insert(key.into(), json!(true));
    }
  }
  if v.i64("end") == Some(1) {
    extra.insert("closed".into(), json!(true));
  }
}
