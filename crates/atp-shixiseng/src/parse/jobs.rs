//! Jobs: search cards, job pages, the rows of a company's jobs, saved jobs
//! and applications, as posts with the shared job keys in `extra`.
//!
//! Internships are paid by the day (`100-150/天`), campus and experienced
//! jobs by the month (`6k-8k/月`): job pages and most lists name the pay
//! as shown (`salary_desc`), search cards as figures, which are written
//! the same way before the shared salary parser reads them.

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::text::{from_beijing, from_secs};
use atp_core::{Post, User, Value, ValueExt, json};

use super::{sanitize, strings, text_of};
use crate::refs;

/// What a job is, by the site's `xiaozhao_type`: 0 and 3 internships
/// (实习), 1 and 2 campus jobs (校招), 4 experienced hires (社招).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recruit {
  Intern,
  Campus,
  Social,
}

impl Recruit {
  pub fn from_code(code: i64) -> Self {
    match code {
      1 | 2 => Self::Campus,
      4 => Self::Social,
      _ => Self::Intern,
    }
  }

  /// By the `type` of saved jobs (`intern`, `xz_intern`, `sz_intern`).
  fn from_saved(kind: &str) -> Self {
    if kind.contains("xz") {
      Self::Campus
    } else if kind.contains("sz") {
      Self::Social
    } else {
      Self::Intern
    }
  }

  /// `recruit_type`.
  pub fn name(self) -> &'static str {
    match self {
      Self::Intern => "intern",
      Self::Campus => "campus",
      Self::Social => "social",
    }
  }

  /// The `type` saving a job takes.
  pub fn saved_type(self) -> &'static str {
    match self {
      Self::Intern => "intern",
      Self::Campus => "xz_intern",
      Self::Social => "sz_intern",
    }
  }
}

/// An amount as the site writes monthly pay: `6k`, `6.5k`, `800`.
fn thousands(n: u64) -> String {
  match n {
    0..1000 => n.to_string(),
    _ if n.is_multiple_of(1000) => format!("{}k", n / 1000),
    _ => format!("{:.1}k", n as f64 / 1000.0),
  }
}

/// The pay of figures: `100-150/天` for internships, `6k-8k/月` for
/// other jobs; nothing when both are 0.
fn pay_text(min: Option<u64>, max: Option<u64>, recruit: Recruit) -> Option<String> {
  let (min, max) = (min.filter(|n| *n > 0), max.filter(|n| *n > 0));
  let show = |n: u64| match recruit {
    Recruit::Intern => n.to_string(),
    _ => thousands(n),
  };
  let range = match (min, max) {
    (Some(min), Some(max)) if min != max => format!("{}-{}", show(min), show(max)),
    (Some(n), _) | (None, Some(n)) => show(n),
    (None, None) => return None,
  };
  let per = if recruit == Recruit::Intern {
    "天"
  } else {
    "月"
  };
  Some(format!("{range}/{per}"))
}

/// A whole number of a field written as a number or a string (`5`, `"5"`).
fn number(v: &Value, path: &str) -> Option<u64> {
  v.u64(path).filter(|n| *n > 0)
}

/// The job keys and Shixiseng's own fields every shape of a job may have.
fn job_fields(extra: &mut Extra, v: &Value, recruit: Recruit) {
  let shown = v.str("salary_desc").or_else(|| {
    let min = number(v, "minsalary").or_else(|| number(v, "minsal"));
    let max = number(v, "maxsalary").or_else(|| number(v, "maxsal"));
    pay_text(min, max, recruit)
  });
  if let Some(pay) = shown.as_deref().and_then(Salary::parse) {
    pay.fill(extra);
  }
  for (key, paths) in [
    ("city", &["city"][..]),
    ("degree", &["degree"]),
    ("category", &["job"]),
    ("address", &["address"]),
  ] {
    if let Some(value) = v.first_str(paths).filter(|s| !s.trim().is_empty()) {
      extra.insert(key.into(), json!(value));
    }
  }
  let part_time = v.bool("is_part_job") == Some(true);
  let job_type = match recruit {
    Recruit::Intern => "intern",
    _ if part_time => "part-time",
    _ => "full-time",
  };
  extra.insert("job_type".into(), json!(job_type));
  extra.insert("recruit_type".into(), json!(recruit.name()));
  for (key, paths) in [
    ("days_per_week", &["day"][..]),
    ("months", &["month", "month_num"]),
  ] {
    if let Some(n) = paths.iter().find_map(|p| number(v, p)) {
      extra.insert(key.into(), json!(n));
    }
  }
  let audience = strings(v, "hope_you");
  if !audience.is_empty() {
    extra.insert("audience".into(), json!(audience));
  }
}

/// The company of a job, with its page.
fn company(v: &Value, id_paths: &[&str], name_paths: &[&str], logo: &str) -> Option<User> {
  let name = v.first_str(name_paths)?;
  let id = v.first_str(id_paths).filter(|id| id.starts_with("com_"));
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    avatar: v.str(logo).filter(|l| l.starts_with("http")),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  };
  for (key, path) in [
    ("industry", "industry"),
    ("size", "scale"),
    ("funding", "stock_status"),
  ] {
    if let Some(value) = v.str(path).filter(|s| !s.trim().is_empty()) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  let tags = strings(v, "c_tags");
  if !tags.is_empty() {
    user.extra.insert("tags".into(), json!(tags));
  }
  Some(user)
}

/// A job post: link, title, the company (also as `company` and
/// `company_*` keys) and the shared fields.
fn job(id: String, v: &Value, recruit: Recruit, author: Option<User>) -> Post {
  let mut post = Post {
    url: Some(refs::job_url(&id)),
    kind: "job".into(),
    title: v.first_str(&["iname", "name"]),
    updated_at: v
      .first_str(&["refresh_time", "refresh"])
      .as_deref()
      .and_then(from_beijing),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  if let Some(company) = &author {
    post.extra.insert("company".into(), json!(company.name));
    for key in ["industry", "size", "funding"] {
      if let Some(value) = company.extra.get(key) {
        post.extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  job_fields(&mut post.extra, v, recruit);
  post.author = author;
  post
}

/// Tags of a job: its perks (`i_tags`, `attraction`), then skills.
fn tags(v: &Value) -> Vec<String> {
  let mut tags = strings(v, "i_tags");
  tags.extend(strings(v, "attraction"));
  tags.extend(strings(v, "skill"));
  tags.extend(strings(v, "skills"));
  let mut seen = std::collections::HashSet::new();
  tags.retain(|t| seen.insert(t.clone()));
  tags
}

/// A card of the job search, its private characters already decoded.
pub fn card(v: &Value, recruit: Recruit) -> Option<Post> {
  let id = v.str("uuid")?;
  let author = company(v, &["c_uuid"], &["cname"], "url");
  let mut post = job(id, v, recruit, author);
  post.tags = tags(v);
  if v.bool("deliver") == Some(true) {
    post.extra.insert("applied".into(), json!(true));
  }
  Some(post)
}

/// A job's page (`interns/info/wxz`): the whole description, pay, city
/// and address, days a week and months, degree, deadline, category,
/// perks and skills, how many applied, the company.
pub fn detail(v: &Value, id: &str) -> Option<Post> {
  let recruit = Recruit::from_code(v.i64("xiaozhao_type").unwrap_or(0));
  let author = company(v, &["cuuid"], &["cname"], "logo");
  let mut post = job(id.to_owned(), v, recruit, author);
  post.title.as_ref()?;
  let parts: Vec<String> = ["info", "job_requirements", "job_other_info"]
    .iter()
    .filter_map(|path| text_of(v, path))
    .collect();
  post.text = (!parts.is_empty()).then(|| parts.join("\n\n"));
  post.tags = tags(v);
  let extra = &mut post.extra;
  if let Some(deadline) = v.str("endtime").filter(|d| !d.starts_with("0000")) {
    extra.insert("deadline".into(), json!(deadline));
  }
  for (key, path) in [("return_offer", "chance"), ("apply_url", "apply_link")] {
    if let Some(value) = v.str(path).filter(|s| !s.trim().is_empty()) {
      extra.insert(key.into(), json!(value));
    }
  }
  // Jobs the site gathered elsewhere name where they came from.
  if let Some(source) = v
    .str("source")
    .filter(|s| s != "实习僧" && !s.trim().is_empty())
  {
    extra.insert("source".into(), json!(source));
  }
  if let Some(n) = v.u64("delivered_num") {
    extra.insert("applicants".into(), json!(n));
  }
  for (key, path) in [
    ("applied", "delivered"),
    ("saved", "collected"),
    ("closed", "overdue"),
  ] {
    if v.str(path).as_deref() == Some("1") {
      extra.insert(key.into(), json!(true));
    }
  }
  Some(post)
}

/// A row of a company's jobs (`company/interns/wxz`).
pub fn company_job(v: &Value, company: &User) -> Option<Post> {
  let id = v.str("uuid")?;
  let recruit = if v.bool("is_xz") == Some(true) {
    Recruit::Campus
  } else {
    Recruit::Intern
  };
  Some(job(id, v, recruit, Some(company.clone())))
}

/// A saved job (`collect/list`): the job, its company and whether it is
/// still taking applications (`status`: `normal`, or the site's word for
/// why not).
pub fn saved(v: &Value) -> Option<Post> {
  let id = v.str("uuid")?;
  let recruit = Recruit::from_saved(&v.str("intern_type").unwrap_or_default());
  let author = company(v, &["company_uuid"], &["company_name"], "url");
  let mut post = job(id, v, recruit, author);
  post.text = v.str("description").filter(|d| !d.trim().is_empty());
  if let Some(status) = v.str("status") {
    post.extra.insert("status".into(), json!(status));
  }
  if v.bool("is_deliver") == Some(true) {
    post.extra.insert("applied".into(), json!(true));
  }
  Some(post)
}

/// The words of the site's application stages.
fn stage_label(status: &str) -> Option<&'static str> {
  Some(match status {
    "delivered" => "投递成功",
    "checked" => "已查看",
    "undeter" => "待定",
    "written" => "通知笔试",
    "notify" => "通知面试",
    "reject" => "不合适",
    "employed" => "已录用",
    _ => return None,
  })
}

/// One of your applications (`deliver/list`): the job, its company, when
/// it was sent and the stage it is at (`status`, `status_label`).
pub fn application(v: &Value) -> Option<Post> {
  let job = v.at("intern");
  let id = job.str("uuid")?;
  let author = company(job, &["company_uuid"], &["company_name"], "logo");
  let mut post = Post {
    url: Some(refs::job_url(&id)),
    kind: "application".into(),
    title: job.first_str(&["intern_name", "name"]),
    created_at: v.i64("time").and_then(from_secs),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = &author {
    extra.insert("company".into(), json!(company.name));
  }
  if let Some(city) = job.str("city") {
    extra.insert("city".into(), json!(city));
  }
  if let Some(status) = v.str("deliver_status") {
    if let Some(label) = stage_label(&status) {
      extra.insert("status_label".into(), json!(label));
    }
    extra.insert("status".into(), json!(status));
  }
  if let Some(application) = v.str("uuid") {
    extra.insert("application_id".into(), json!(application));
  }
  if v.bool("is_unread") == Some(true) {
    extra.insert("unread".into(), json!(true));
  }
  post.author = author;
  Some(post)
}
