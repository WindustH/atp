//! 51job payloads → the shared model: the search's job cards (with each
//! job's whole description) and what ATP reads off job and company pages
//! ([`crate::pages`]) as posts with the shared job keys in `extra`, and
//! companies as users.
//!
//! The pay is the site's own words (`1.5-2万·14薪`, `25-40万/年`, `面议`),
//! read by the shared salary parser. Recruiters (names, titles, pictures,
//! whether they are online), contact details and the chat buttons' state
//! are dropped from every payload, `raw` included ([`sanitize`]).

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::text::{from_beijing, html_to_text};
use atp_core::{Post, User, Value, ValueExt, json};

use crate::filters::JobType;
use crate::refs;

/// Keys about contacting recruiters, dropped wherever they appear.
const DROP_KEYS: &[&str] = &[
  "contactAllowed",
  "contactDay",
  "contactTime",
  "isCommunicate",
  "allowChatOnline",
  "isAllowChat",
  "isOnline",
  "userInteractionOnlineLabel",
];
/// Parts of keys that hold contact details.
const CONTACT_PARTS: &[&str] = &["phone", "mobile", "wechat", "weixin", "email"];

/// Whether `key` is about the recruiter (`hrName`, `hrUid`,
/// `smallHrLogoUrl` …) or a contact.
fn dropped(key: &str) -> bool {
  let recruiter = key.contains("Hr")
    || (key.starts_with("hr") && key[2..].starts_with(|c: char| c.is_ascii_uppercase()));
  let lower = key.to_lowercase();
  recruiter || DROP_KEYS.contains(&key) || CONTACT_PARTS.iter().any(|part| lower.contains(part))
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
fn text(v: &Value, path: &str) -> Option<String> {
  v.str(path)
    .map(|s| s.trim().to_owned())
    .filter(|s| !s.is_empty())
}

/// The company of a card: its code, short and full name, logo, page,
/// ownership, size and industry.
fn company(v: &Value) -> Option<User> {
  let name = text(v, "companyName").or_else(|| text(v, "fullCompanyName"))?;
  let code = text(v, "encCoId");
  let mut user = User {
    url: code.as_deref().map(refs::company_url),
    avatar: text(v, "companyLogo").filter(|l| l.starts_with("http")),
    id: code.unwrap_or_default(),
    name,
    ..User::default()
  };
  for (key, path) in [
    ("full_name", "fullCompanyName"),
    ("type", "companyTypeString"),
    ("size", "companySizeString"),
    ("industry", "companyIndustryType1Str"),
  ] {
    if let Some(value) = text(v, path) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  if !user.extra.contains_key("industry")
    && let Some(industry) = text(v, "coIndustryText")
  {
    user.extra.insert("industry".into(), json!(industry));
  }
  Some(user)
}

/// The job keys and 51job's own fields of a card.
fn job_fields(extra: &mut Extra, v: &Value) {
  if let Some(pay) = text(v, "provideSalaryString")
    .as_deref()
    .and_then(Salary::parse)
  {
    pay.fill(extra);
  }
  let area = v.at("jobAreaLevelDetail");
  let city = text(area, "cityString").or_else(|| text(v, "jobAreaString"));
  for (key, value) in [
    ("city", city),
    ("district", text(area, "districtString")),
    ("landmark", text(v, "landmarkString")),
    ("experience", text(v, "workYearString")),
    ("degree", text(v, "degreeString")),
    ("work_type", text(v, "termStr")),
  ] {
    if let Some(value) = value {
      extra.insert(key.into(), json!(value));
    }
  }
  let kind = text(v, "termStr").as_deref().and_then(JobType::from_word);
  let kind = kind.or_else(|| (v.bool("isIntern") == Some(true)).then_some(JobType::Intern));
  if let Some(kind) = kind {
    extra.insert("job_type".into(), json!(kind.name()));
  }
  if v.bool("isRemoteWork") == Some(true) {
    extra.insert("remote".into(), json!(true));
  }
  let majors: Vec<String> = ["major1Str", "major2Str"]
    .iter()
    .filter_map(|path| text(v, path))
    .collect();
  if !majors.is_empty() {
    extra.insert("majors".into(), json!(majors));
  }
  for (key, path) in [("applied", "isApply"), ("closed", "isExpire")] {
    if v.bool(path) == Some(true) {
      extra.insert(key.into(), json!(true));
    }
  }
  // Jobs of companies with a career site of their own on 51job link there.
  if let Some(href) = text(v, "jobHref").filter(|h| !h.contains("jobs.51job.com")) {
    extra.insert("apply_url".into(), json!(href));
  }
}

/// Skills and benefits, as tags: the card's tags without the experience
/// and degree it also lists there.
fn tags(v: &Value) -> Vec<String> {
  let shown = [text(v, "workYearString"), text(v, "degreeString")];
  let mut tags: Vec<String> = v
    .list("jobTags")
    .iter()
    .filter_map(|t| t.as_str())
    .map(|t| t.trim().to_owned())
    .filter(|t| !t.is_empty() && !shown.iter().any(|s| s.as_deref() == Some(t.as_str())))
    .collect();
  let mut seen = std::collections::HashSet::new();
  tags.retain(|t| seen.insert(t.clone()));
  tags
}

/// A description as text: typed with line breaks, or written in HTML by
/// some companies.
fn description(text: &str) -> String {
  if text.contains("</") || text.contains("<br") {
    html_to_text(text)
  } else {
    text.to_owned()
  }
}

/// A card of the search: the job with its whole description.
pub fn card(v: &Value) -> Option<Post> {
  let id = text(v, "jobId")?;
  let author = company(v);
  let mut post = Post {
    url: Some(refs::job_url(&id, text(v, "hrefAreaPinYin").as_deref())),
    kind: "job".into(),
    title: text(v, "jobName"),
    text: text(v, "jobDescribe").map(|t| description(&t)),
    created_at: text(v, "issueDateString").as_deref().and_then(from_beijing),
    updated_at: text(v, "updateDateTime").as_deref().and_then(from_beijing),
    tags: tags(v),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  if let Some(company) = &author {
    post.extra.insert("company".into(), json!(company.name));
    for key in ["full_name", "type", "size", "industry"] {
      if let Some(value) = company.extra.get(key) {
        post.extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  job_fields(&mut post.extra, v);
  post.author = author;
  Some(post)
}

/// Words of the degrees job pages name.
const DEGREES: &[&str] = &[
  "初中",
  "高中",
  "中专",
  "中技",
  "大专",
  "本科",
  "硕士",
  "博士",
  "学历不限",
  "无学历要求",
];

/// The line under a job page's title (`西安-雁塔区 | 3-4年经验 | 本科 |
/// 招1人 | 10-18发布`) into the job keys.
fn job_line(extra: &mut Extra, line: &str) {
  let mut parts = line
    .split(['|', '\u{a0}'])
    .map(str::trim)
    .filter(|p| !p.is_empty());
  if let Some(place) = parts.next() {
    let (city, district) = place.split_once('-').unwrap_or((place, ""));
    extra.insert("city".into(), json!(city));
    if !district.is_empty() {
      extra.insert("district".into(), json!(district));
    }
  }
  for part in parts {
    if part.contains("经验") || part.ends_with("年") || part.contains("应届") {
      extra.insert("experience".into(), json!(part));
    } else if DEGREES.iter().any(|d| part.contains(d)) {
      extra.insert("degree".into(), json!(part));
    } else if let Some(n) = part
      .strip_prefix('招')
      .and_then(|p| p.strip_suffix('人'))
      .and_then(|n| n.parse::<u64>().ok())
    {
      extra.insert("openings".into(), json!(n));
    }
  }
}

/// `职能类别：Java开发工程师`, `上班地址：…` rows as `(label, value)`.
fn row(text: &str) -> Option<(&str, String)> {
  let (label, value) = text.split_once(['：', ':'])?;
  let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
  (!value.is_empty()).then(|| (label.trim(), value))
}

/// The company named on a job page: its page, ownership, size, industry.
fn page_company(page: &Value) -> Option<User> {
  let name = text(page, "company")?;
  let code = text(page, "company_url").and_then(|url| refs::company(&url).ok());
  let mut user = User {
    url: code.as_deref().map(refs::company_url),
    id: code.unwrap_or_default(),
    name,
    ..User::default()
  };
  let tags = text(page, "company_tags").unwrap_or_default();
  let tags: Vec<&str> = tags
    .lines()
    .map(str::trim)
    .filter(|t| !t.is_empty())
    .collect();
  for (key, value) in ["type", "size", "industry"].iter().zip(tags) {
    user.extra.insert((*key).into(), json!(value));
  }
  Some(user)
}

/// What a job page shows ([`crate::pages`]).
pub fn job_page(page: &Value, number: &str, url: &str) -> Option<Post> {
  let mut post = Post {
    id: number.to_owned(),
    url: Some(url.to_owned()),
    kind: "job".into(),
    title: Some(text(page, "title")?),
    text: text(page, "text"),
    tags: page
      .list("welfare")
      .iter()
      .filter_map(|t| t.as_str().map(str::to_owned))
      .collect(),
    raw: Some(page.clone()),
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(pay) = text(page, "salary").as_deref().and_then(Salary::parse) {
    pay.fill(extra);
  }
  if let Some(line) = text(page, "meta") {
    job_line(extra, &line);
  }
  for item in page.list("rows") {
    match item.as_str().and_then(row) {
      Some(("职能类别", value)) => extra.insert("category".into(), json!(value)),
      Some(("上班地址", value)) => extra.insert("address".into(), json!(value)),
      _ => None,
    };
  }
  if let Some(company) = page_company(page) {
    extra.insert("company".into(), json!(company.name));
    for key in ["type", "size", "industry"] {
      if let Some(value) = company.extra.get(key) {
        extra.insert(format!("company_{key}"), value.clone());
      }
    }
    post.author = Some(company);
  }
  Some(post)
}

/// A job card of a company page: the JSON of its `sensorsdata` attribute
/// (`jobId`, `jobTitle`, `jobSalary`, `jobArea`, `jobYear`, `jobDegree`,
/// `funcType`) and its link.
fn company_card(card: &Value, company: &User) -> Option<Post> {
  let data: Value = serde_json::from_str(&card.str("data")?).ok()?;
  let id = text(&data, "jobId")?;
  let href = text(card, "href");
  let url = href
    .as_deref()
    .and_then(|h| refs::job(h).ok())
    .map_or_else(|| refs::job_url(&id, None), |(_, url)| url);
  let mut post = Post {
    url: Some(url),
    kind: "job".into(),
    title: text(&data, "jobTitle"),
    raw: Some(data.clone()),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(pay) = text(&data, "jobSalary").as_deref().and_then(Salary::parse) {
    pay.fill(extra);
  }
  for (key, path) in [
    ("city", "jobArea"),
    ("experience", "jobYear"),
    ("degree", "jobDegree"),
    ("category", "funcType"),
  ] {
    if let Some(value) = text(&data, path) {
      extra.insert(key.into(), json!(value));
    }
  }
  extra.insert("company".into(), json!(company.name));
  post.author = Some(company.clone());
  Some(post)
}

/// What a company page shows ([`crate::pages`]): the company and the jobs
/// it lists.
pub fn company_page(page: &Value, code: &str) -> Option<(User, Vec<Post>)> {
  let mut user = User {
    id: code.to_owned(),
    url: Some(refs::company_url(code)),
    name: text(page, "name")?,
    avatar: text(page, "logo").filter(|l| l.starts_with("http")),
    bio: text(page, "intro"),
    ..User::default()
  };
  let info: Vec<&str> = page
    .list("info")
    .iter()
    .filter_map(|i| i.as_str())
    .collect();
  for (key, value) in ["type", "size", "industry"].iter().zip(info) {
    user.extra.insert((*key).into(), json!(value));
  }
  let mut seen = std::collections::HashSet::new();
  let jobs: Vec<Post> = page
    .list("cards")
    .iter()
    .filter_map(|card| company_card(card, &user))
    .filter(|post| seen.insert(post.id.clone()))
    .collect();
  user.raw = Some(page.clone());
  Some((user, jobs))
}
