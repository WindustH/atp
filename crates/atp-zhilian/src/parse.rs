//! Zhilian payloads → the shared model: job cards (the search, the mobile
//! search page) and job pages (`positionDetail`) as posts with the shared
//! job keys in `extra`, companies as users.
//!
//! A job names its pay as the site shows it (`salary60` 1.5-2.5万, with
//! `salaryCount` ·14薪; 150-250元/天; 面议), read by the shared salary
//! parser. Recruiters (name, title, avatar, activity), the phone button's
//! state and anything named like a contact are dropped from every
//! payload, `raw` included ([`sanitize`]).

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::text::{from_beijing, html_to_text};
use atp_core::{Post, User, Value, ValueExt, json};

use crate::filters::JobType;
use crate::refs;

/// Keys of recruiters and their contacts, dropped wherever they appear.
const DROP_KEYS: &[&str] = &[
  "staffCard",
  "staff",
  "staffId",
  "recommender",
  "preferredHrInfo",
  "displayPhoneNumber",
  "alreadyCallPhone",
  "callProcess",
  "imSection",
  "imSessionInfoDetail",
];
/// Parts of keys that hold contact details.
const CONTACT_PARTS: &[&str] = &["phone", "mobile", "wechat", "weixin", "email"];

fn dropped(key: &str) -> bool {
  let lower = key.to_lowercase();
  DROP_KEYS.contains(&key) || CONTACT_PARTS.iter().any(|part| lower.contains(part))
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

/// Strings of a list of strings or of objects naming one under `key`.
fn names(list: &[Value], key: &str) -> Vec<String> {
  list
    .iter()
    .filter_map(|item| item.as_str().map(str::to_owned).or_else(|| item.str(key)))
    .map(|name| name.trim().to_owned())
    .filter(|name| !name.is_empty())
    .collect()
}

/// The pay as shown: `1.5-2.5万·14薪`.
fn salary_text(v: &Value) -> Option<String> {
  let shown = v.first_str(&["salary60", "salary"])?;
  Some(match v.str("salaryCount") {
    Some(count) if !shown.contains('薪') => format!("{shown}·{count}"),
    _ => shown,
  })
}

/// The company of a card or job page, with its page.
fn company(v: &Value) -> Option<User> {
  let name = v.str("companyName")?;
  let number = v.str("companyNumber").filter(|n| n != "0");
  let mut user = User {
    url: number.as_deref().map(refs::company_url),
    avatar: v.str("companyLogo").filter(|l| l.starts_with("http")),
    id: number.unwrap_or_default(),
    name,
    ..User::default()
  };
  for (key, paths) in [
    ("size", &["companySize"][..]),
    ("property", &["propertyName", "property"]),
    ("industry", &["industryName", "industry"]),
  ] {
    if let Some(value) = v.first_str(paths).filter(|s| !s.trim().is_empty()) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}

/// The job keys and Zhilian's own fields of a card or job page.
fn job_fields(extra: &mut Extra, v: &Value) {
  if let Some(pay) = salary_text(v).as_deref().and_then(Salary::parse) {
    pay.fill(extra);
  }
  for (key, paths) in [
    ("city", &["workCity", "positionWorkCity"][..]),
    ("district", &["cityDistrict", "positionCityDistrict"]),
    ("experience", &["workingExp", "positionWorkingExp"]),
    ("degree", &["education"]),
    ("work_type", &["workType", "emplType"]),
    ("category", &["subJobTypeLevelName"]),
    ("address", &["workAddress"]),
  ] {
    if let Some(value) = v.first_str(paths).filter(|s| !s.trim().is_empty()) {
      extra.insert(key.into(), json!(value));
    }
  }
  let code = v.first_str(&["workTypeCode", "emplTypeCode"]);
  if let Some(kind) = code.as_deref().and_then(JobType::job_key) {
    extra.insert("job_type".into(), json!(kind));
  }
  if let Some(n) = v.u64("recruitNumber").filter(|n| *n > 0) {
    extra.insert("openings".into(), json!(n));
  }
  if v.bool("hasAppliedPosition") == Some(true) {
    extra.insert("applied".into(), json!(true));
  }
}

/// Skills, then benefits, as tags.
fn tags(v: &Value) -> Vec<String> {
  let mut tags = names(v.list("jobSkillTags"), "name");
  tags.extend(names(v.list("skillLabel"), "value"));
  tags.extend(names(v.list("jobKeyword.keywords"), "itemValue"));
  tags.extend(names(v.list("welfareTags"), "name"));
  let mut seen = std::collections::HashSet::new();
  tags.retain(|t| seen.insert(t.clone()));
  tags
}

/// A description as text: typed with line breaks, or written in HTML
/// (`<div>`s, `<br>`s) by some companies.
fn description(text: &str) -> String {
  if text.contains("</") || text.contains("<br") {
    html_to_text(text)
  } else {
    text.trim().to_owned()
  }
}

/// A job: a search card, or the flat `positionDetail` of a job page.
fn job(v: &Value, text: Option<String>) -> Option<Post> {
  let text = text.map(|t| description(&t));
  let number = v.first_str(&["number", "positionNumber"])?;
  let author = company(v);
  let mut post = Post {
    url: Some(refs::job_url(&number)),
    kind: "job".into(),
    title: v.first_str(&["name", "positionName"]),
    text: text.filter(|t| !t.trim().is_empty()),
    created_at: v
      .first_str(&["firstPublishTime", "publishTime", "positionPublishTime"])
      .as_deref()
      .and_then(from_beijing),
    updated_at: v.str("updateTime").as_deref().and_then(from_beijing),
    tags: tags(v),
    raw: Some(sanitize(v)),
    id: number,
    ..Post::default()
  };
  if let Some(company) = &author {
    post.extra.insert("company".into(), json!(company.name));
    for key in ["size", "property", "industry"] {
      if let Some(value) = company.extra.get(key) {
        post.extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  job_fields(&mut post.extra, v);
  post.author = author;
  Some(post)
}

/// A card of the search: the job, with its description when the card
/// brings it (`jobDetailData`).
pub fn card(v: &Value) -> Option<Post> {
  let text = v
    .str("jobDetailData.position.desc.description")
    .or_else(|| v.str("jobSummary"));
  job(v, text)
}

/// A job page's `positionDetail`: the whole description, the place, the
/// company, how applications are taken.
pub fn detail(state: &Value) -> Option<Post> {
  let flat = state.at("positionDetail");
  let mut post = job(flat, flat.str("description"))?;
  let extra = &mut post.extra;
  let how = state.at("state");
  if how.str("applyType").as_deref() == Some("4") {
    extra.insert("apply_url".into(), json!(flat.str("positionUrl")));
  }
  if flat.bool("atsHumanResource") == Some(true) {
    extra.insert("company_ats".into(), json!(true));
  }
  if flat.i64("jobStatus") == Some(4) {
    extra.insert("closed".into(), json!(true));
  }
  let about = state.at("companyDetail");
  if let Some(author) = post.author.as_mut() {
    for (key, path) in [
      ("subtitle", "companySubTitle"),
      ("open_jobs", "onlinePositionNumbers"),
    ] {
      if let Some(value) = about.str(path).filter(|s| !s.is_empty()) {
        author.extra.insert(key.into(), json!(value));
      }
    }
  }
  post.raw = Some(sanitize(state));
  Some(post)
}

/// The first object in `v` (depth first) that `wanted` accepts.
fn find<'a>(v: &'a Value, wanted: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
  if wanted(v) {
    return Some(v);
  }
  match v {
    Value::Object(map) => map.values().find_map(|child| find(child, wanted)),
    Value::Array(items) => items.iter().find_map(|child| find(child, wanted)),
    _ => None,
  }
}

/// A company's page: the object of its data naming the company by its
/// number, read with the keys the job pages' company data has (`base`:
/// name, logo, size, the line under the name, introduction, industry,
/// city, open jobs).
pub fn company_page(state: &Value, number: &str) -> Option<User> {
  let about = find(state, &|v| {
    v.str("companyNumber").as_deref() == Some(number) && v.str("companyName").is_some()
  })?;
  let mut user = User {
    id: number.to_owned(),
    url: Some(refs::company_url(number)),
    name: about.str("companyName")?,
    avatar: about.str("companyLogo").filter(|l| l.starts_with("http")),
    bio: about
      .first_str(&["companyDescription", "description"])
      .filter(|d| !d.trim().is_empty()),
    location: about.first_str(&["cityName", "address"]),
    raw: Some(sanitize(state)),
    ..User::default()
  };
  user.stats.posts = about.u64("onlinePositionNumbers");
  for (key, paths) in [
    ("short_name", &["companyShotName", "companyShortName"][..]),
    ("subtitle", &["companySubTitle"]),
    ("size", &["companySize"]),
    (
      "industry",
      &["industryNameLevel", "industryName", "industry"],
    ),
    ("property", &["property", "propertyName"]),
    ("funding", &["financingStageName"]),
  ] {
    if let Some(value) = about.first_str(paths).filter(|s| !s.trim().is_empty()) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}
