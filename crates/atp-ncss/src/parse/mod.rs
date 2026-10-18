//! NCSS payloads → the shared model: job cards of the search, of a job
//! fair and of a company's page as posts with the shared job keys in
//! `extra`, companies as users, job fairs as posts; job and company pages
//! through [`page`].
//!
//! A card names its pay in thousands a month (`lowMonthPay` 8,
//! `highMonthPay` 14), shown as `8-14K/月`, or `面议` for 0 and 0; the
//! text is built as the page shows it and read by the shared salary
//! parser.

mod page;

pub use page::{JobPage, LoginState, company, job, login_state, token};

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::text::{date_only, from_beijing, from_millis};
use atp_core::{Post, User, Value, ValueExt, json};
use jiff::Timestamp;

use crate::refs;

/// What the site names jobs it posts itself.
pub const PLATFORM: &str = "国家大学生就业服务平台";

/// A number as the page writes it: `8`, `6.7`.
fn figure(n: f64) -> String {
  if n.fract() == 0.0 {
    format!("{n:.0}")
  } else {
    format!("{n}")
  }
}

/// The pay of a card as the page shows it.
fn salary_text(v: &Value) -> Option<String> {
  if let Some(text) = v.str("salary") {
    let text = text.trim();
    return Some(if text == "0k-0k" { "面议" } else { text }.to_owned());
  }
  let (low, high) = (v.f64("lowMonthPay")?, v.f64("highMonthPay")?);
  if low == 0.0 && high == 0.0 {
    return Some("面议".into());
  }
  Some(format!("{}-{}K/月", figure(low), figure(high)))
}

/// A time as cards write it: milliseconds, or `2026-10-18 10:52` in Beijing.
pub fn time(v: &Value) -> Option<Timestamp> {
  if let Some(n) = v.as_i64().filter(|n| *n > 0) {
    return from_millis(n);
  }
  v.as_str().and_then(from_beijing)
}

/// The company of a card: with its page when it has one (`recId` is `-`
/// for companies without).
fn card_company(v: &Value) -> Option<User> {
  let name = v.first_str(&["recName", "corpName"])?;
  let id = v
    .first_str(&["corpId", "recId"])
    .filter(|id| id != "-" && !id.is_empty());
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    avatar: v
      .first_str(&["recLogo", "logo"])
      .filter(|l| l.starts_with("http")),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  };
  for (key, path) in [("size", "recScale"), ("property", "recProperty")] {
    if let Some(value) = v.str(path).filter(|s| s != "-") {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}

/// Where a job was posted: the platform itself, or the school, the
/// province's bureau or the partner site named.
fn source(v: &Value) -> String {
  let id = v.str("sourcesName").filter(|s| s != "0" && s != "null");
  match (id, v.str("sourcesNameCh")) {
    (Some(_), Some(name)) => name,
    _ => PLATFORM.into(),
  }
}

/// The kind of place a job was posted on (`sourcesType`).
fn source_type(code: &str) -> Option<&'static str> {
  Some(match code {
    "0" => "job site",
    "1" | "11" => "province",
    "12" => "school",
    _ => return None,
  })
}

/// Labels a company gave itself: `五险一金，带薪年假` or comma-separated.
fn labels(text: &str) -> Vec<String> {
  text
    .split(['，', ','])
    .map(str::trim)
    .filter(|t| !t.is_empty())
    .map(str::to_owned)
    .collect()
}

/// The job keys and NCSS's own fields of a card.
fn card_fields(extra: &mut Extra, v: &Value) {
  if let Some(pay) = salary_text(v).as_deref().and_then(Salary::parse) {
    pay.fill(extra);
  }
  for (key, paths) in [
    ("city", &["areaName", "areaCodeName"][..]),
    ("degree", &["degreeName"]),
    ("major", &["major"]),
  ] {
    if let Some(value) = v.first_str(paths).filter(|s| !s.trim().is_empty()) {
      extra.insert(key.into(), json!(value));
    }
  }
  if let Some(kind) = v
    .str("jobType")
    .as_deref()
    .and_then(crate::filters::JobType::from_site)
  {
    extra.insert("job_type".into(), json!(kind.name()));
  }
  if let Some(n) = v.u64("headCount").filter(|n| *n > 0) {
    extra.insert("openings".into(), json!(n));
  }
  if v.str("recruitType").as_deref() == Some("1") {
    extra.insert("announcement".into(), json!(true));
  }
  if v.str("keyUnits").as_deref() == Some("1") {
    extra.insert("key_field".into(), json!(true));
  }
  if v.str("memberLevel").as_deref() == Some("2") {
    extra.insert("featured".into(), json!(true));
  }
  // Cards of a company's page carry no source: the company posted them here.
  if v.get("sourcesName").is_some() {
    extra.insert("source".into(), json!(source(v)));
  }
  if let Some(kind) = v.str("sourcesType").as_deref().and_then(source_type) {
    extra.insert("source_type".into(), json!(kind));
  }
}

/// A job card: the search's, a job fair's or a company page's.
pub fn card(v: &Value) -> Option<Post> {
  let id = v.str("jobId")?;
  let author = card_company(v);
  let mut post = Post {
    url: Some(refs::job_url(&id)),
    kind: "job".into(),
    title: v.str("jobName").map(|t| t.trim().to_owned()),
    created_at: time(v.at("publishDate")),
    updated_at: time(v.at("updateDate")),
    tags: v.str("recTags").map(|t| labels(&t)).unwrap_or_default(),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  if let Some(company) = &author {
    post.extra.insert("company".into(), json!(company.name));
    for key in ["size", "property"] {
      if let Some(value) = company.extra.get(key) {
        post.extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  card_fields(&mut post.extra, v);
  post.author = author;
  Some(post)
}

/// The day a time falls on in Beijing.
fn day(v: &Value) -> Option<jiff::civil::Date> {
  let beijing = jiff::tz::TimeZone::fixed(jiff::tz::offset(8));
  Some(time(v)?.to_zoned(beijing).date())
}

/// What `publishStatus` says of a fair.
fn fair_status(code: &str) -> Option<&'static str> {
  Some(match code {
    "0" => "open",
    "1" => "upcoming",
    "2" | "3" => "ended",
    _ => return None,
  })
}

/// A job fair (招聘会): the site's own, with its page, or another site's
/// (`jobFairType` 1), linked there.
pub fn fair(v: &Value) -> Option<Post> {
  let id = v.str("fairId")?;
  let outside = v.str("jobFairType").as_deref() == Some("1");
  let url = if outside {
    v.str("outSideLink").filter(|l| l.starts_with("http"))
  } else {
    Some(refs::fair_url(&id))
  };
  let mut post = Post {
    url,
    kind: "fair".into(),
    title: v.str("fairName"),
    text: v.str("introductions").filter(|t| !t.trim().is_empty()),
    created_at: day(v.at("startDate")).and_then(date_only),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  for (key, path) in [("starts", "startDate"), ("ends", "endDate")] {
    if let Some(date) = day(v.at(path)) {
      extra.insert(key.into(), json!(date.to_string()));
    }
  }
  if let Some(sponsor) = v.str("sponsor") {
    extra.insert("sponsor".into(), json!(sponsor));
  }
  if let Some(status) = v.str("publishStatus").as_deref().and_then(fair_status) {
    extra.insert("status".into(), json!(status));
  }
  if outside {
    extra.insert("elsewhere".into(), json!(true));
  }
  counts(extra, v, ["corpNum", "jobNum", "employNums"]);
  Some(post)
}

/// The companies, jobs and openings of a fair, when it names them.
pub fn counts(extra: &mut Extra, v: &Value, paths: [&str; 3]) {
  for (key, path) in ["companies", "jobs", "openings"].into_iter().zip(paths) {
    if let Some(n) = v.u64(path).filter(|n| *n > 0) {
      extra.insert(key.into(), json!(n));
    }
  }
}

/// A company taking part in a fair.
pub fn fair_company(v: &Value) -> Option<User> {
  let name = v.str("name")?;
  // Companies without a page of their own have the id `-`.
  let id = v.str("id").filter(|id| id != "-");
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    name,
    avatar: v
      .str("logoId")
      .map(|logo| format!("https://t1.chei.com.cn/student/ncss/cdn/job/img/{logo}.png")),
    raw: Some(v.clone()),
    id: id.unwrap_or_default(),
    ..User::default()
  };
  if v.str("keyUnits").as_deref() == Some("1") {
    user.extra.insert("key_field".into(), json!(true));
  }
  if v.str("memberLevel").as_deref() == Some("2") {
    user.extra.insert("featured".into(), json!(true));
  }
  Some(user)
}
