//! Jobs (search cards and job pages), 校招 schedule entries and your
//! applications as posts with the shared job keys in `extra`.
//!
//! A job names its pay as numbers the page formats: `salaryType` 1 is
//! yuan a day (internships, `300-350元/天`), 2 thousands a month with the
//! months paid (`15-25K·14薪`), 3 ten-thousands a year (`20-40万/年`); 0 to
//! 9999999 is 薪资面议. The text is built as the page shows it and read by
//! the shared salary parser.

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::{Post, User, Value, ValueExt, json};

use super::{day, millis, sanitize_job};
use crate::refs;

/// 学历 codes of `eduLevel`.
fn degree(code: i64) -> Option<&'static str> {
  Some(match code {
    1000 => "小学",
    2000 => "初中",
    3000 => "高中",
    4000 => "专科",
    5000 => "本科",
    5500 => "双学位",
    6000 => "硕士",
    7000 => "博士",
    8000 => "博士后",
    _ => return None,
  })
}

/// 经验 codes of `workYearType` (experienced hires).
fn experience(code: i64) -> Option<&'static str> {
  Some(match code {
    0 => "在校生",
    1 => "1年以内",
    2 => "1-3年",
    3 => "3-5年",
    4 => "5-10年",
    5 => "10年以上",
    _ => return None,
  })
}

/// `recruitType`: 1 校招, 2 实习, 3 社招.
fn recruit(kind: i64) -> Option<(&'static str, &'static str)> {
  Some(match kind {
    1 => ("campus", "full-time"),
    2 => ("intern", "intern"),
    3 => ("social", "full-time"),
    _ => return None,
  })
}

/// `jobOffer` of internships.
fn return_offer(code: i64) -> Option<&'static str> {
  Some(match code {
    0 => "转正面议",
    1 => "有转正",
    2 => "无转正",
    _ => return None,
  })
}

/// A number as the page writes it: `15`, `1.5`.
fn figure(n: f64) -> String {
  if n.fract() == 0.0 {
    format!("{n:.0}")
  } else {
    format!("{n}")
  }
}

/// The pay as the page shows it.
fn salary_text(v: &Value) -> Option<String> {
  if let Some(shown) = v.str("salaryShow").filter(|s| !s.trim().is_empty()) {
    return Some(shown);
  }
  let (min, max) = (v.f64("salaryMin")?, v.f64("salaryMax")?);
  let intern = v.i64("recruitType") == Some(2);
  let kind = v.i64("salaryType").unwrap_or(if intern { 1 } else { 2 });
  // 0-999K a month is what some companies enter for pay to be agreed.
  if min == 0.0 && (max >= 9_999_999.0 || (kind == 2 && max >= 999.0)) {
    return Some("薪资面议".into());
  }
  let range = format!("{}-{}", figure(min), figure(max));
  Some(match (kind, v.u64("salaryMonth").filter(|m| *m > 0)) {
    (1, _) => format!("{range}元/天"),
    (3, _) => format!("{range}万/年"),
    (_, Some(months)) => format!("{range}K·{months}薪"),
    (_, None) => format!("{range}K"),
  })
}

/// The company of a job: the job page's `jobCompany`, else the card's
/// `recommendInternCompany`.
fn company(v: &Value, page: Option<&Value>) -> Option<User> {
  let card = v.at("recommendInternCompany");
  let id = v
    .str("companyId")
    .filter(|id| id != "0")
    .or_else(|| card.str("companyId"))
    .or_else(|| page.and_then(|p| p.str("tagId")))?;
  let name = page
    .and_then(|p| p.str("companyName"))
    .or_else(|| card.first_str(&["companyShortName", "companyName"]))
    .or_else(|| v.str("companyName"))?;
  let mut user = User {
    url: Some(refs::company_url(&id)),
    avatar: page
      .and_then(|p| p.str("picUrl"))
      .or_else(|| card.str("picUrl")),
    id,
    name,
    ..User::default()
  };
  let industry = page
    .and_then(|p| p.str("category"))
    .or_else(|| card.str("industryTagNameList.0"))
    .or_else(|| v.str("industryName"));
  let size = page
    .and_then(|p| p.str("personScales"))
    .or_else(|| card.str("personScales"));
  for (key, value) in [("industry", industry), ("size", size)] {
    if let Some(value) = value.filter(|s| !s.is_empty()) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}

/// The cities of a job, as the card lists them.
fn cities(v: &Value) -> Option<String> {
  let list: Vec<String> = v
    .list("jobCityList")
    .iter()
    .filter_map(|c| c.as_str().map(str::to_owned))
    .collect();
  if list.is_empty() {
    v.str("jobCity").map(|c| c.replace(',', "/"))
  } else {
    Some(list.join("/"))
  }
}

/// The job keys and Nowcoder's own fields of a job.
fn job_fields(extra: &mut Extra, v: &Value) {
  if let Some(pay) = salary_text(v).as_deref().and_then(Salary::parse) {
    pay.fill(extra);
  }
  if let Some(city) = cities(v) {
    extra.insert("city".into(), json!(city));
  }
  if let Some((recruit, job_type)) = v.i64("recruitType").and_then(recruit) {
    extra.insert("recruit_type".into(), json!(recruit));
    extra.insert("job_type".into(), json!(job_type));
  }
  if let Some(level) = v.i64("eduLevel").and_then(degree) {
    extra.insert("degree".into(), json!(level));
  }
  for (key, path) in [
    ("graduation", "graduationYear"),
    ("address", "jobAddress"),
    ("career", "careerJobName"),
    ("apply_url", "redirectExternalUrl"),
  ] {
    if let Some(value) = v.str(path).filter(|s| !s.trim().is_empty()) {
      extra.insert(key.into(), json!(value));
    }
  }
  job_extras(extra, v);
}

/// The deadline, what internships and experienced hires add, and how fast
/// the company answers.
fn job_extras(extra: &mut Extra, v: &Value) {
  if let Some(deadline) = day(v, "deliverEnd") {
    extra.insert("deadline".into(), json!(deadline));
  }
  match v.i64("recruitType") {
    Some(2) => {
      for (key, path) in [
        ("days_per_week", "durationDays"),
        ("months", "durationMonths"),
      ] {
        if let Some(n) = v.u64(path).filter(|n| *n > 0) {
          extra.insert(key.into(), json!(n));
        }
      }
      if let Some(offer) = v.i64("jobOffer").and_then(return_offer) {
        extra.insert("return_offer".into(), json!(offer));
      }
    }
    Some(3) => {
      if let Some(years) = v.i64("workYearType").and_then(experience) {
        extra.insert("experience".into(), json!(years));
      }
    }
    _ => {}
  }
  for (key, path) in [
    ("processing_rate", "avgProcessRate"),
    ("processing_days", "avgProcessDay"),
  ] {
    if let Some(n) = v.u64(path).filter(|n| *n > 0) {
      extra.insert(key.into(), json!(n));
    }
  }
}

/// The description of a job page: duties (`infos`) and requirements, from
/// the `ext` JSON text.
fn description(v: &Value) -> Option<String> {
  let ext: Value = serde_json::from_str(&v.str("ext")?).ok()?;
  let parts: Vec<String> = [("职位描述", "infos"), ("职位要求", "requirements")]
    .iter()
    .filter_map(|(label, key)| {
      let text = ext.str(key)?.replace("\r\n", "\n");
      let text = text.trim();
      (!text.is_empty()).then(|| format!("{label}\n{text}"))
    })
    .collect();
  (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// A job: a search card, or a job page's `detail` with its `jobCompany`
/// (`page`), which also brings the description.
pub fn job(v: &Value, page: Option<&Value>) -> Option<Post> {
  let id = v.str("id")?;
  let author = company(v, page);
  let mut post = Post {
    url: Some(refs::job_url(&id)),
    kind: "job".into(),
    title: v.first_str(&["jobName", "jobTitle"]),
    text: page.and_then(|_| description(v)),
    created_at: millis(v, "createTime"),
    updated_at: millis(v, "refreshTime").or_else(|| millis(v, "updateTime")),
    tags: v
      .str("jobKeys")
      .map(|k| {
        k.split(',')
          .map(str::trim)
          .filter(|k| !k.is_empty())
          .map(str::to_owned)
          .collect()
      })
      .unwrap_or_default(),
    raw: Some(sanitize_job(v)),
    id,
    ..Post::default()
  };
  if let Some(company) = &author {
    post.extra.insert("company".into(), json!(company.name));
    for key in ["industry", "size"] {
      if let Some(value) = company.extra.get(key) {
        post.extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  job_fields(&mut post.extra, v);
  post.author = author;
  Some(post)
}

/// A saved job (`user/collect/job`): a job card, or a row wrapping one.
pub fn saved_job(v: &Value) -> Option<Post> {
  for key in ["job", "jobInfo", "data"] {
    if v.at(key).is_object() {
      return job(v.at(key), None);
    }
  }
  if v.at("id").is_null() && !v.at("jobId").is_null() {
    let mut row = v.clone();
    row["id"] = v.at("jobId").clone();
    return job(&row, None);
  }
  job(v, None)
}

/// One of your applications (`/v2/resume/deliver-list`): the job, its
/// company, the stage it is at (`status_label`) and the steps so far,
/// newest first as the site lists them (`history`).
pub fn application(v: &Value) -> Post {
  let job_id = v.str("jobId").filter(|id| id != "0");
  let steps = v.list("transferList");
  let mut post = Post {
    id: job_id
      .clone()
      .or_else(|| v.str("deliverId"))
      .unwrap_or_default(),
    url: job_id.as_deref().map(refs::job_url),
    kind: "application".into(),
    title: v.first_str(&["jobName", "jobTitle"]),
    created_at: steps.last().and_then(|s| millis(s, "createTime")),
    updated_at: steps
      .first()
      .and_then(|s| millis(s, "createTime"))
      .or_else(|| millis(v, "latestUpdateTime")),
    raw: Some(sanitize_job(v)),
    ..Post::default()
  };
  let extra = &mut post.extra;
  for (key, path) in [
    ("company", "companyName"),
    ("company_id", "companyId"),
    ("application_id", "deliverId"),
    ("status_label", "currentStageName"),
  ] {
    if let Some(value) = v.str(path).filter(|s| s != "0") {
      extra.insert(key.into(), json!(value));
    }
  }
  job_fields(extra, v);
  let history: Vec<Value> = steps
    .iter()
    .filter_map(|s| {
      let status = s.str("subStageName")?;
      Some(json!({"status": status, "at": millis(s, "createTime")}))
    })
    .collect();
  if !history.is_empty() {
    extra.insert("history".into(), json!(history));
  }
  post
}
