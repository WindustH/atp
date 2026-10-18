//! The pages NCSS renders on the server: a job
//! (`/student/jobs/<id>/detail.html`) and a company
//! (`/student/jobs/<id>/corp.html`), plus what every page says about the
//! login and the tokens of the forms it offers.
//!
//! A job page has the title, the job type (`[全职]`), the pay as shown
//! (`10k-15k`), the degree, the openings, the majors wanted, where the job
//! was posted (来源), the place, the whole description and the company
//! with its industry, fields, ownership, size, website and address. A
//! company page has the name, logo, introduction and the same facts, the
//! industry as a code.

use std::sync::LazyLock;

use atp_core::jobs::Salary;
use atp_core::model::Extra;
use atp_core::text::html_to_text;
use atp_core::{Post, User, json};
use regex::Regex;

use super::PLATFORM;
use crate::codes::industry_name;
use crate::filters::JobType;
use crate::refs;

fn re(pattern: &str) -> Regex {
  Regex::new(pattern).expect("valid pattern")
}

static TITLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<li class="job-title[^"]*"[^>]*>([^<]*)</li>"#));
static JOB_TYPE: LazyLock<Regex> = LazyLock::new(|| re(r"<li>\[<span>([^<]*)</span>\]</li>"));
static SALARY_BOX: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<ul class="salary clearfix">(.*?)</ul>"#));
static SPAN: LazyLock<Regex> = LazyLock::new(|| re(r"<span>([^<]+)</span>"));
static OPENINGS: LazyLock<Regex> = LazyLock::new(|| re(r"招聘\s*<span><span>(\d+)</span>人"));
static MAJOR: LazyLock<Regex> = LazyLock::new(|| re(r#"<div class="major" title="([^"]*)""#));
static SOURCE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<span class="source-sp"(?: data-sourcesType="([^"]*)")?>([^<]*)</span>"#));
static PLACE: LazyLock<Regex> = LazyLock::new(|| re(r#"<div class="site-tag">([^<]*)</div>"#));
static BODY: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<(div|pre) class="mainContent[^"]*">(.*?)</(?:div|pre)>"#));
static CORP_NAME: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<span id="realCorpName">([^<]*)</span>"#));
static CORP_ID: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<p id="corpId" class="hide">([^<]*)</p>"#));
static CORP_IMG: LazyLock<Regex> = LazyLock::new(|| re(r#"<img class="corp-img" src="([^"]*)""#));
static JOB_FACT: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)<span class="ico fl">\s*<span class="iconfont [^"]*"></span>([^<]+)</span>\s*<span[^>]*>(.*?)</span>"#,
  )
});
static COMPANY_NAME: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<span class="corp-name" title="[^"]*">([^<]*)</span>"#));
static COMPANY_LOGO: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<img class="company-logo" src="([^"]*)""#));
static COMPANY_INTRO: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<div class="text company-prife">(.*?)</div>"#));
static COMPANY_FACT: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)<span class="fl"><span class="iconfont [^"]*"></span>([^<]+)</span>\s*<div[^>]*>(.*?)</div>"#,
  )
});
static MASK: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<div id="(mask4|maskUser)" class="hide maskmodal">([^<]*)</div>"#));

/// The first capture of `re` in `html`, as text.
fn capture(re: &Regex, html: &str) -> Option<String> {
  let text = html_to_text(&re.captures(html)?[1]);
  (!text.is_empty()).then_some(text)
}

/// HTML whose line breaks are its own (descriptions typed with returns)
/// as text: read as preformatted, `<br>`s and entities included.
fn preformatted(html: &str) -> String {
  html_to_text(&format!("<pre>{html}</pre>"))
}

/// Who a page says is logged in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginState {
  Nobody,
  Student,
  /// An employer's or a manager's account.
  Other,
}

pub fn login_state(html: &str) -> Option<LoginState> {
  let mut logged = None;
  let mut student = None;
  for m in MASK.captures_iter(html) {
    let yes = m[2].trim() != "false";
    match &m[1] {
      "mask4" => logged = Some(yes),
      _ => student = Some(yes),
    }
  }
  Some(match (logged?, student.unwrap_or(true)) {
    (false, _) => LoginState::Nobody,
    (true, true) => LoginState::Student,
    (true, false) => LoginState::Other,
  })
}

/// The token of a form the page offers: `(name, value)` of its hidden
/// `<input id="csrf-…">`.
pub fn token(html: &str, id: &str) -> Option<(String, String)> {
  let pattern = format!(
    r#"<input id="{}\s*" type="hidden" value="([^"]*)" name="([^"]*)""#,
    regex::escape(id)
  );
  let m = Regex::new(&pattern).ok()?.captures(html)?;
  Some((m[2].to_owned(), m[1].to_owned()))
}

/// A job page and what applying needs to know.
pub struct JobPage {
  pub post: Post,
  /// Where the job was posted (来源), the platform's own name included.
  pub source: String,
}

impl JobPage {
  /// Posted on the platform itself (not by a school, a province or a
  /// partner site).
  pub fn own(&self) -> bool {
    self.source == PLATFORM
  }
}

/// The company column of a job page.
fn page_company(html: &str) -> Option<User> {
  let name = capture(&CORP_NAME, html)?;
  let id = capture(&CORP_ID, html).filter(|id| id != "-");
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    avatar: capture(&CORP_IMG, html).filter(|src| src.starts_with("http")),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  };
  for m in JOB_FACT.captures_iter(html) {
    let key = match m[1].trim() {
      "所属行业" => "industry",
      "涉及领域" => "fields",
      "公司性质" => "property",
      "公司规模" => "size",
      "公司网址" => "website",
      "所在地址" => "address",
      _ => continue,
    };
    let value = html_to_text(&m[2]);
    if !value.is_empty() && value != "--" {
      user.extra.insert(key.into(), json!(value));
    }
  }
  Some(user)
}

/// The line under a job's title (pay, degree, openings, the time
/// updated) and the majors and place below it.
fn facts(extra: &mut Extra, html: &str) {
  let line: Vec<String> = SALARY_BOX
    .captures(html)
    .map(|m| {
      SPAN
        .captures_iter(&m[1])
        .map(|s| html_to_text(&s[1]))
        .collect()
    })
    .unwrap_or_default();
  let pay = line.first().map(|p| if p == "0k-0k" { "面议" } else { p });
  if let Some(pay) = pay.and_then(Salary::parse) {
    pay.fill(extra);
  }
  if let Some(degree) = line
    .get(1)
    .filter(|d| !d.chars().any(|c| c.is_ascii_digit()))
  {
    extra.insert("degree".into(), json!(degree));
  }
  if let Some(n) = OPENINGS
    .captures(html)
    .and_then(|m| m[1].parse::<u64>().ok())
  {
    extra.insert("openings".into(), json!(n));
  }
  for (key, re) in [("major", &*MAJOR), ("city", &*PLACE)] {
    if let Some(value) = capture(re, html) {
      extra.insert(key.into(), json!(value));
    }
  }
}

/// A job's page.
pub fn job(html: &str, id: &str) -> Option<JobPage> {
  let title = capture(&TITLE, html)?;
  let author = page_company(html);
  let mut post = Post {
    id: id.to_owned(),
    url: Some(refs::job_url(id)),
    kind: "job".into(),
    title: Some(title),
    text: BODY
      .captures(html)
      .map(|m| preformatted(&m[2]))
      .filter(|t| !t.is_empty()),
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = &author {
    extra.insert("company".into(), json!(company.name));
    for key in ["industry", "property", "size"] {
      if let Some(value) = company.extra.get(key) {
        extra.insert(format!("company_{key}"), value.clone());
      }
    }
  }
  if let Some(kind) = capture(&JOB_TYPE, html)
    .as_deref()
    .and_then(JobType::from_site)
  {
    extra.insert("job_type".into(), json!(kind.name()));
  }
  facts(extra, html);
  let source = SOURCE
    .captures(html)
    .map(|m| html_to_text(&m[2]))
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| PLATFORM.into());
  extra.insert("source".into(), json!(source));
  post.author = author;
  Some(JobPage { post, source })
}

/// Industries as the company page writes them, codes (`000304`, several
/// joined with commas) named as its script names them; other text as it is.
fn industries(value: &str) -> String {
  let parts: Vec<&str> = value
    .split(',')
    .map(str::trim)
    .filter(|p| !p.is_empty())
    .collect();
  if parts.is_empty() || !parts.iter().all(|p| industry_name(p).is_some()) {
    return value.to_owned();
  }
  let names: Vec<&str> = parts.iter().filter_map(|p| industry_name(p)).collect();
  names.join("、")
}

/// A company's page.
pub fn company(html: &str, id: &str) -> Option<User> {
  let name = capture(&COMPANY_NAME, html)?;
  let mut user = User {
    id: id.to_owned(),
    url: Some(refs::company_url(id)),
    avatar: capture(&COMPANY_LOGO, html).filter(|src| src.starts_with("http")),
    bio: COMPANY_INTRO
      .captures(html)
      .map(|m| html_to_text(&m[1]))
      .filter(|t| !t.is_empty()),
    name,
    ..User::default()
  };
  for m in COMPANY_FACT.captures_iter(html) {
    let key = match m[1].trim() {
      "所属行业" => "industry",
      "涉及领域" => "fields",
      "公司性质" => "property",
      "公司规模" => "size",
      "公司网址" => "website",
      "所在地址" => "address",
      _ => continue,
    };
    let value = industries(&html_to_text(&m[2]));
    if !value.is_empty() && value != "--" {
      user.extra.insert(key.into(), json!(value));
    }
  }
  if let Some(address) = user.extra.get("address").and_then(|a| a.as_str()) {
    user.location = Some(address.to_owned());
  }
  Some(user)
}
