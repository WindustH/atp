//! Shixiseng payloads → the shared model: jobs ([`jobs`]) as posts with
//! the shared job keys in `extra`, companies as users.
//!
//! Phone numbers (`tel`), the e-mail address some companies take
//! applications at (`deliver_email`), anything named like a contact, and
//! the advertising and Q&A blocks of job pages (people's names and
//! schools) are dropped from every payload, `raw` included ([`sanitize`]).

mod jobs;

pub use jobs::{Recruit, application, card, company_job, detail, saved};

use atp_core::text::html_to_text;
use atp_core::{User, Value, ValueExt, json};

use crate::refs;

/// Keys dropped wherever they appear.
const DROP_KEYS: &[&str] = &["tel", "deliver_email", "qa_info", "course", "linkedin_info"];
/// Parts of keys that hold contact details.
const CONTACT_PARTS: &[&str] = &["phone", "mobile", "email", "wechat", "weixin"];

fn dropped(key: &str) -> bool {
  let lower = key.to_lowercase();
  DROP_KEYS.contains(&key) || CONTACT_PARTS.iter().any(|part| lower.contains(part))
}

/// The payload without contact details and advertising.
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

/// Strings of a list of strings, or of a JSON text holding one
/// (`"[\"朝气蓬勃\"]"`, as company tags and cities come).
pub fn strings(v: &Value, path: &str) -> Vec<String> {
  let parsed;
  let list = match v.at(path) {
    Value::String(text) => {
      parsed = serde_json::from_str::<Value>(text).unwrap_or_default();
      parsed.as_array().map(Vec::as_slice).unwrap_or_default()
    }
    _ => v.list(path),
  };
  list
    .iter()
    .filter_map(Value::as_str)
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(str::to_owned)
    .collect()
}

/// Text that may be HTML (`info` with `info_is_html`).
pub fn text_of(v: &Value, path: &str) -> Option<String> {
  let text = v.str(path)?;
  let text = if text.contains('<') || text.contains('&') {
    html_to_text(&text)
  } else {
    text.trim().to_owned()
  };
  (!text.is_empty()).then_some(text)
}

/// Copy the non-empty strings of `v` named in `fields` into `user.extra`.
fn fields(user: &mut User, v: &Value, fields: &[(&str, &str)]) {
  for (key, path) in fields {
    if let Some(value) = v.str(path).filter(|s| !s.trim().is_empty()) {
      user.extra.insert((*key).into(), json!(value.trim()));
    }
  }
}

/// A company's page (`company/info/wxz`): name, logo, introduction, city,
/// address, size, industry, funding, ownership, registration, website,
/// tags and how many jobs it has open.
pub fn company(v: &Value, id: &str) -> Option<User> {
  let name = v.first_str(&["name", "cname"])?;
  let mut user = User {
    id: id.to_owned(),
    url: Some(refs::company_url(id)),
    name,
    avatar: v.str("logo").filter(|l| l.starts_with("http")),
    bio: text_of(v, "info").or_else(|| v.str("description")),
    location: v.first_str(&["city", "address"]),
    raw: Some(sanitize(v)),
    ..User::default()
  };
  let interns = v.u64("interns_num").unwrap_or(0);
  let campus = v.u64("xiaozhao_job_num").unwrap_or(0);
  user.stats.posts = Some(interns + campus);
  fields(
    &mut user,
    v,
    &[
      ("legal_name", "cname"),
      ("subtitle", "description"),
      ("address", "address"),
      ("size", "scale"),
      ("industry", "industry"),
      ("funding", "stock_status"),
      ("ownership", "nature"),
      ("company_type", "com_type"),
      ("founded", "start_time"),
      ("capital", "reg_capi"),
      ("website", "wurl"),
    ],
  );
  for (key, n) in [("internships", interns), ("campus_jobs", campus)] {
    user.extra.insert(key.into(), json!(n));
  }
  let tags = strings(v, "tags");
  if !tags.is_empty() {
    user.extra.insert("tags".into(), json!(tags));
  }
  Some(user)
}

/// A company of the company search (`/company/search`).
pub fn company_card(v: &Value) -> Option<User> {
  let id = v.str("uuid")?;
  let mut user = User {
    url: Some(refs::company_url(&id)),
    name: v.first_str(&["name", "law_name"])?,
    avatar: v.str("logo_url").filter(|l| l.starts_with("http")),
    bio: v.str("description"),
    location: Some(strings(v, "city").join("/")).filter(|c| !c.is_empty()),
    raw: Some(sanitize(v)),
    id,
    ..User::default()
  };
  user.stats.posts = v.u64("intern_num");
  fields(
    &mut user,
    v,
    &[
      ("legal_name", "law_name"),
      ("size", "scale"),
      ("industry", "industry"),
    ],
  );
  let tags = strings(v, "tags");
  if !tags.is_empty() {
    user.extra.insert("tags".into(), json!(tags));
  }
  Some(user)
}

/// The logged-in account (`baseinfo`): id, name and avatar; the phone
/// number and e-mail address it also tells are left out.
pub fn account(v: &Value) -> Option<User> {
  let id = v.str("uuid")?;
  let mut user = User {
    name: v.str("name").unwrap_or_else(|| id.clone()),
    avatar: v.str("head").filter(|h| h.starts_with("http")),
    raw: Some(sanitize(&strip_account(v))),
    id,
    ..User::default()
  };
  if v.str("flag").as_deref() == Some("company") {
    user.extra.insert("recruiter".into(), json!(true));
  }
  Some(user)
}

/// The account without its login name, which is often the phone number.
fn strip_account(v: &Value) -> Value {
  let mut v = v.clone();
  if let Some(map) = v.as_object_mut() {
    map.remove("account");
    map.remove("area_code");
  }
  v
}
