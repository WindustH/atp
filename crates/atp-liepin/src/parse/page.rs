//! The pages Liepin renders on the server: a job (`/job/<n>.shtml`,
//! `/a/<n>.shtml`) and a company (`/company/<n>/`).
//!
//! A job page carries `var $CONFIG = {jobId, jobKind, compId, compName …}`,
//! a schema.org `JobPosting` (dates, employer, place, pay, industry,
//! experience, degree) and the HTML around them: the pay as shown, the
//! line of properties (city, experience, degree, 招N人), the benefits, the
//! whole description, the recruiter's name and title, the company's
//! industry and size. A company page has its name, logo, industry, size,
//! stage, benefits, introduction, number of open jobs, an `Organization`
//! with the address, and the business register (工商信息).

use std::sync::LazyLock;

use atp_core::jobs::{Period, Salary};
use atp_core::model::Extra;
use atp_core::{Post, User, UserStats, Value, ValueExt, json};
use regex::Regex;

use super::{image, sanitize, time};
use crate::refs;

fn re(pattern: &str) -> Regex {
  Regex::new(pattern).expect("valid pattern")
}

static BREAK: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)<br\s*/?>"));
static TAG: LazyLock<Regex> = LazyLock::new(|| re(r"<[^>]*>"));
static ENTITY: LazyLock<Regex> = LazyLock::new(|| re(r"&#(x?)([0-9a-fA-F]+);"));
static CONFIG: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)var \$CONFIG = (\{.*?\});"));
static LD: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<script type="application/ld\+json">(.*?)</script>"#));
static SALARY: LazyLock<Regex> = LazyLock::new(|| re(r#"<span class="salary">([^<]*)</span>"#));
static PROPERTIES: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<div class="job-properties">(.*?)</div>"#));
static SPAN: LazyLock<Regex> = LazyLock::new(|| re(r#"<span(?: class="([^"]*)")?>([^<]*)</span>"#));
static BENEFITS: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)<section class="job-apply-container-desc">.*?<div class="labels">(.*?)</div>"#)
});
static INTRO: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<dd data-selector="job-intro-content">(.*?)</dd>"#));
static OTHER: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<dt>其他信息</dt>(.*?)</dl>"));
static DD: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<dd[^>]*>(.*?)</dd>"));
static RECRUITER: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)<section class="recruiter-container">.*?<span class="name">([^<]*)</span>.*?<div class="title-box">\s*<span>([^<]*)</span>"#,
  )
});
static COMPANY_INTRO: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)<section class="company-intro-container"[^>]*>.*?<div class="inner[^"]*">(.*?)</div>"#)
});
static LABELED: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)<span class="label">([^<]*)</span>\s*<span class="text">([^<]*)</span>"#)
});
static COMPANY_NAME: LazyLock<Regex> =
  LazyLock::new(|| re(r#"data-selector="company-name">([^<]*)<"#));
static COMPANY_LOGO: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<img class="logo company-logo-64" src="([^"]+)""#));
static COMPANY_LINE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<div class="name-right">.*?<p>(.*?)</p>"#));
static COMPANY_TAG: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="tags-item(?:-text)?">\s*<span>([^<]+)</span>"#));
static COMPANY_JOBS: LazyLock<Regex> = LazyLock::new(|| re(r"职位\((\d+)\)"));
static COMPANY_TEXT: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<div class="inner-text">(.*?)</div>"#));
static REGISTER: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)<span class="name">([^<]+)</span>\s*</p>\s*<p class="text">([^<]*)</p>"#)
});

/// The first capture of `re` in `html`, as text.
fn capture(re: &Regex, html: &str) -> Option<String> {
  re.captures(html)
    .map(|m| text(&m[1]))
    .filter(|s| !s.is_empty())
}

/// HTML as plain text: the descriptions keep the line breaks they were
/// written with (not `<br>`s), so tags are dropped and entities decoded
/// without reflowing the text.
fn text(html: &str) -> String {
  let html = BREAK.replace_all(html, "\n");
  let html = TAG.replace_all(&html, "");
  let html = ENTITY.replace_all(&html, |m: &regex::Captures| {
    let code = if m[1].is_empty() {
      m[2].parse().ok()
    } else {
      u32::from_str_radix(&m[2], 16).ok()
    };
    code
      .and_then(char::from_u32)
      .map(String::from)
      .unwrap_or_default()
  });
  let html = html
    .replace("&quot;", "\"")
    .replace("&#39;", "'")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&nbsp;", " ")
    .replace("&amp;", "&");
  let lines: Vec<&str> = html.lines().map(str::trim).collect();
  lines.join("\n").trim().to_owned()
}

/// JSON with raw line breaks and tabs inside its strings (as the pages'
/// `ld+json` has them) made valid.
fn lenient(json: &str) -> Option<Value> {
  let mut out = String::with_capacity(json.len());
  let (mut quoted, mut escaped) = (false, false);
  for c in json.chars() {
    match c {
      '\n' if quoted => out.push_str("\\n"),
      '\r' if quoted => out.push_str("\\r"),
      '\t' if quoted => out.push_str("\\t"),
      _ => out.push(c),
    }
    if escaped {
      escaped = false;
    } else if c == '\\' {
      escaped = quoted;
    } else if c == '"' {
      quoted = !quoted;
    }
  }
  serde_json::from_str(&out).ok()
}

/// The `ld+json` block of a schema.org type.
fn ld(html: &str, kind: &str) -> Value {
  LD.captures_iter(html)
    .filter_map(|m| lenient(&m[1]))
    .find(|v| v.str("@type").as_deref() == Some(kind))
    .unwrap_or_default()
}

/// The publication times of the Baidu `cambrian` block: `upDate`.
fn updated(html: &str) -> Option<jiff::Timestamp> {
  LD.captures_iter(html)
    .filter_map(|m| lenient(&m[1]))
    .find_map(|v| time(v.at("upDate")))
}

/// `$CONFIG` of a job page.
pub fn config(html: &str) -> Option<Value> {
  let json = CONFIG.captures(html)?;
  serde_json::from_str::<Value>(&json[1]).ok()
}

/// A job page.
pub fn job(html: &str, url: &str) -> Option<Post> {
  let config = config(html)?;
  let posting = ld(html, "JobPosting");
  let mut post = Post {
    id: refs::job_id(url).or_else(|| config.str("jobId"))?,
    kind: "job".into(),
    title: posting.str("title").or_else(|| config.str("jobTitle")),
    text: capture(&INTRO, html).or_else(|| posting.str("description")),
    url: Some(url.to_owned()),
    created_at: time(posting.at("datePosted")),
    updated_at: updated(html),
    tags: benefits(html),
    author: employer(&config, &posting),
    raw: Some(json!({"config": sanitize(&config), "posting": sanitize(&posting)})),
    ..Post::default()
  };
  fill_job(&mut post, html, &config, &posting);
  Some(post)
}

fn benefits(html: &str) -> Vec<String> {
  let Some(block) = BENEFITS.captures(html) else {
    return Vec::new();
  };
  SPAN
    .captures_iter(&block[1])
    .map(|m| text(&m[2]))
    .filter(|s| !s.is_empty())
    .collect()
}

/// The company that hires: `$CONFIG` and the `JobPosting`'s employer.
fn employer(config: &Value, posting: &Value) -> Option<User> {
  let org = posting.at("hiringOrganization");
  let id = config
    .str("compId")
    .or_else(|| org.str("sameAs").and_then(|u| refs::company(&u).ok()));
  let name = config.str("compName").or_else(|| org.str("name"))?;
  Some(User {
    url: id.as_deref().map(refs::company_url),
    avatar: org.str("logo"),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  })
}

/// The shared job keys and Liepin's own fields of a job page.
fn fill_job(post: &mut Post, html: &str, config: &Value, posting: &Value) {
  let extra = &mut post.extra;
  if let Some(company) = &post.author {
    extra.insert("company".into(), json!(company.name));
  }
  let shown = capture(&SALARY, html);
  match shown.as_deref().and_then(Salary::parse) {
    Some(pay) => pay.fill(extra),
    None => posting_pay(extra, posting),
  }
  let place = posting.at("jobLocation.address");
  if let Some(city) = place.str("addressLocality") {
    extra.insert("city".into(), json!(city));
  }
  if let Some(address) = place.str("streetAddress") {
    extra.insert("address".into(), json!(address));
  }
  for (key, path) in [
    ("experience", "experienceRequirements"),
    ("degree", "educationRequirements"),
    ("industry", "industry"),
  ] {
    if let Some(value) = posting.str(path) {
      extra.insert(key.into(), json!(value));
    }
  }
  properties(extra, html);
  if let Some(kind) = config.str("jobKind") {
    extra.insert("job_kind".into(), json!(kind));
    extra.insert("posted_by".into(), json!(super::posted_by(&kind)));
  }
  if let Some(id) = config.str("jobId") {
    extra.insert("job_id".into(), json!(id));
  }
  if let Some(m) = RECRUITER.captures(html) {
    extra.insert(
      "recruiter".into(),
      json!({"name": text(&m[1]), "title": Some(text(&m[2])).filter(|t| !t.is_empty())}),
    );
  }
  page_company(extra, html);
}

/// The pay of the `JobPosting` when the page shows none, by the period
/// of its `unitText` (weekly pay is left out).
fn posting_pay(extra: &mut Extra, posting: &Value) {
  let pay = posting.at("baseSalary");
  let period = match pay.str("value.unitText").as_deref() {
    Some("HOUR") => Period::Hour,
    Some("DAY") => Period::Day,
    Some("MONTH") => Period::Month,
    Some("YEAR") => Period::Year,
    _ => return,
  };
  let figure = |path: &str| pay.f64(path).filter(|n| *n > 0.0);
  let mut salary = Salary::new(figure("value.minValue"), figure("value.maxValue"), period);
  if let Some(currency) = pay.str("currency") {
    salary.currency = currency;
  }
  salary.fill(extra);
}

/// The line under the title: 招N人, 学生可投, 实习, the time updated.
fn properties(extra: &mut Extra, html: &str) {
  let Some(block) = PROPERTIES.captures(html) else {
    return;
  };
  for m in SPAN.captures_iter(&block[1]) {
    let value = text(&m[2]);
    let class = m.get(1).map_or("", |c| c.as_str());
    if class == "recruit-cnt" {
      let openings = value.trim_start_matches('招').trim_end_matches('人');
      if let Ok(n) = openings.parse::<u64>() {
        extra.insert("openings".into(), json!(n));
      }
    } else if value.contains("实习") {
      extra.insert("job_type".into(), json!("intern"));
    } else if value == "学生可投" {
      extra.insert("students".into(), json!(true));
    }
  }
}

/// What the page says about the company: its introduction (headhunter
/// jobs), industry, size and the other labeled lines (语言要求 …).
fn page_company(extra: &mut Extra, html: &str) {
  if let Some(intro) = capture(&COMPANY_INTRO, html) {
    extra.insert("company_intro".into(), json!(intro));
  }
  for m in LABELED.captures_iter(html) {
    let key = match m[1].trim().trim_end_matches('：') {
      "企业行业" => "company_industry",
      "人数规模" => "company_size",
      _ => continue,
    };
    extra.insert(key.into(), json!(text(&m[2])));
  }
  if let Some(other) = OTHER.captures(html) {
    let lines: Vec<String> = DD
      .captures_iter(&other[1])
      .map(|m| text(&m[1]))
      .filter(|s| !s.is_empty())
      .collect();
    if !lines.is_empty() {
      extra.insert("other".into(), json!(lines));
    }
  }
}

/// A company page.
pub fn company(html: &str, id: &str) -> Option<User> {
  let name = capture(&COMPANY_NAME, html)?;
  let org = ld(html, "Organization");
  let mut user = User {
    id: id.to_owned(),
    url: Some(refs::company_url(id)),
    avatar: COMPANY_LOGO.captures(html).and_then(|m| image(&m[1])),
    bio: capture(&COMPANY_TEXT, html),
    location: org.str("address.addressLocality"),
    stats: UserStats {
      posts: COMPANY_JOBS.captures(html).and_then(|m| m[1].parse().ok()),
      ..UserStats::default()
    },
    raw: Some(sanitize(&org)),
    name,
    ..User::default()
  };
  company_line(&mut user, html);
  let tags: Vec<String> = COMPANY_TAG
    .captures_iter(html)
    .map(|m| text(&m[1]))
    .collect();
  if !tags.is_empty() {
    user.extra.insert("benefits".into(), json!(tags));
  }
  if let Some(address) = org.str("address.streetAddress") {
    user.extra.insert("address".into(), json!(address));
  }
  register(&mut user, html);
  Some(user)
}

/// `互联网·IT服务·10000人以上·美股上市`: industries, size and stage.
fn company_line(user: &mut User, html: &str) {
  let Some(line) = capture(&COMPANY_LINE, html) else {
    return;
  };
  let mut industries = Vec::new();
  for part in line.split('·').map(str::trim).filter(|p| !p.is_empty()) {
    if part.ends_with('人') || part.ends_with("人以上") {
      user.extra.insert("size".into(), json!(part));
    } else if ["轮", "上市", "融资", "天使"]
      .iter()
      .any(|w| part.contains(w))
    {
      user.extra.insert("stage".into(), json!(part));
    } else {
      industries.push(part.to_owned());
    }
  }
  if !industries.is_empty() {
    user.extra.insert("industries".into(), json!(industries));
  }
}

/// The business register (工商信息) the page carries: the registered
/// name, founding date, capital, type and state.
fn register(user: &mut User, html: &str) {
  for m in REGISTER.captures_iter(html) {
    let key = match m[1].trim() {
      "企业全称" => "full_name",
      "成立时间" => "founded",
      "注册资本" => "capital",
      "企业类型" => "company_type",
      "经营状态" => "status",
      _ => continue,
    };
    let value = text(&m[2]);
    if !value.is_empty() {
      user.extra.insert(key.into(), json!(value));
    }
  }
}
