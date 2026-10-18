//! A job's page (`jobPosting/<id>`): the top card (title, company with
//! its link and logo, place, `15 hours ago`, the applicants), the base
//! pay where LinkedIn shows it, the description, the criteria (seniority
//! level, employment type, job function, industries) and how one applies
//! (on LinkedIn, or on the company's site).
//!
//! The page also holds a sign-in form, "see who was hired" and "find a
//! referral" (people); only the company's number is taken from their
//! links (`facetCurrentCompany`), nothing else of them.

use std::sync::LazyLock;

use atp_core::model::Extra;
use atp_core::text::{html_to_text, parse_count};
use atp_core::{Post, Value, json};
use regex::Regex;

use super::{ago, attr, capture, employer, line, pay, pay_in, re, read_parts, remote, text_value};
use crate::refs;

static TITLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h[12] class="top-card-layout__title[^"]*">(.*?)</h[12]>"#));
static COMPANY: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="topcard__flavor">(.*?)</span>"#));
static COMPANY_LINK: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<a class="topcard__org-name-link[^"]*"[^>]*?href="([^"]+)""#));
static PLACE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="topcard__flavor topcard__flavor--bullet">(.*?)</span>"#));
static POSTED: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="posted-time-ago__text[^"]*">(.*?)</span>"#));
static APPLICANTS: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)class="num-applicants__caption[^"]*"[^>]*>(.*?)</(?:span|figcaption)>"#)
});
static DESCRIPTION: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)class="show-more-less-html__markup[^"]*"[^>]*>(.*?)</div>\s*(?:<button|</section>)"#)
});
static CRITERION: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)<h3 class="description__job-criteria-subheader">(.*?)</h3>\s*<span class="description__job-criteria-text[^"]*">(.*?)</span>"#,
  )
});
static SALARY: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="[^"]*compensation__salary[^"]*"[^>]*>(.*?)</div>"#));
static COMPANY_ID: LazyLock<Regex> = LazyLock::new(|| re(r"facetCurrentCompany(?:%3D|=)(\d+)"));
static LOGO: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="artdeco-entity-image[^"]*"[^>]*data-delayed-url="([^"]+)""#));
static APPLY_URL: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<code id="applyUrl"[^>]*><!--"([^"]+)"-->"#));
static EXACT: LazyLock<Regex> = LazyLock::new(|| re(r"^([\d,]+) applicants?$"));

/// The parts of the top card and the description.
struct Page {
  title: String,
  company: Option<String>,
  link: Option<String>,
  place: Option<String>,
  posted: Option<String>,
  applicants: Option<String>,
  description: Option<String>,
  shown_pay: Option<String>,
  criteria: Vec<(String, String)>,
}

impl Page {
  fn read(html: &str) -> Option<Self> {
    Some(Self {
      title: capture(&TITLE, html)?,
      company: capture(&COMPANY, html),
      link: COMPANY_LINK.captures(html).map(|c| attr(&c[1])),
      place: capture(&PLACE, html),
      posted: capture(&POSTED, html),
      applicants: capture(&APPLICANTS, html),
      description: DESCRIPTION
        .captures(html)
        .map(|c| html_to_text(&c[1]))
        .filter(|t| !t.is_empty()),
      shown_pay: capture(&SALARY, html),
      criteria: CRITERION
        .captures_iter(html)
        .map(|c| (line(&c[1]), line(&c[2])))
        .filter(|(name, value)| !name.is_empty() && !value.is_empty())
        .collect(),
    })
  }

  /// What was read, as `raw`.
  fn raw(&self) -> Value {
    let criteria: serde_json::Map<String, Value> = self
      .criteria
      .iter()
      .map(|(name, value)| (name.clone(), json!(value)))
      .collect();
    read_parts(&[
      ("title", Some(json!(self.title))),
      ("company", text_value(&self.company)),
      ("company_link", text_value(&self.link)),
      ("location", text_value(&self.place)),
      ("posted", text_value(&self.posted)),
      ("applicants", text_value(&self.applicants)),
      ("salary", text_value(&self.shown_pay)),
      (
        "criteria",
        (!criteria.is_empty()).then_some(Value::Object(criteria)),
      ),
    ])
  }
}

/// A job's page; `None` when it is not one.
pub fn job(html: &str, id: &str) -> Option<Post> {
  let page = Page::read(html)?;
  let url = refs::job_url(id);
  let logo = LOGO.captures(html).map(|c| attr(&c[1]));
  let mut post = Post {
    id: id.to_owned(),
    kind: "job".into(),
    url: Some(url.clone()),
    created_at: page.posted.as_deref().and_then(ago),
    author: page
      .company
      .as_deref()
      .and_then(|name| employer(name, page.link.as_deref(), logo)),
    raw: Some(page.raw()),
    ..Post::default()
  };
  let extra = &mut post.extra;
  top(extra, &page);
  criteria(extra, &page.criteria);
  let salary = page
    .shown_pay
    .as_deref()
    .and_then(pay)
    .or_else(|| pay_in(&page.title, page.description.as_deref()));
  if let Some(salary) = salary {
    salary.fill(extra);
  }
  if let Some(id) = COMPANY_ID.captures(html) {
    extra.insert("company_id".into(), json!(&id[1]));
  }
  apply(extra, html, &url);
  post.title = Some(page.title);
  post.text = page.description;
  Some(post)
}

/// The keys of the top card: company, place, remote, the applicants,
/// whether the job was posted again.
fn top(extra: &mut Extra, page: &Page) {
  if let Some(company) = &page.company {
    extra.insert("company".into(), json!(company));
  }
  if let Some(place) = &page.place {
    extra.insert("city".into(), json!(place));
  }
  if remote(&page.title, page.place.as_deref()) {
    extra.insert("remote".into(), json!(true));
  }
  if let Some(shown) = &page.applicants {
    let exact = EXACT.captures(shown).and_then(|c| parse_count(&c[1]));
    if let Some(n) = exact {
      extra.insert("applicants".into(), json!(n));
    }
    extra.insert("applicants_text".into(), json!(shown));
  }
  if let Some(posted) = &page.posted {
    extra.insert("posted".into(), json!(posted));
    if posted.to_lowercase().starts_with("reposted") {
      extra.insert("reposted".into(), json!(true));
    }
  }
}

/// The criteria: seniority level (`experience`), employment type
/// (`employment_type`, and `job_type` where one fits), job function,
/// industries.
fn criteria(extra: &mut Extra, criteria: &[(String, String)]) {
  for (name, value) in criteria {
    match name.to_lowercase().as_str() {
      "seniority level" if !value.eq_ignore_ascii_case("not applicable") => {
        extra.insert("experience".into(), json!(value));
      }
      "employment type" => {
        extra.insert("employment_type".into(), json!(value));
        if let Some(word) = job_type(value) {
          extra.insert("job_type".into(), json!(word));
        }
      }
      "job function" => {
        extra.insert("job_function".into(), json!(value));
      }
      "industries" => {
        extra.insert("industries".into(), json!(value));
      }
      _ => {}
    }
  }
}

/// The shared `job_type` of LinkedIn's employment types, where one fits
/// (not Temporary, Volunteer or Other).
fn job_type(employment: &str) -> Option<&'static str> {
  match employment.trim().to_lowercase().as_str() {
    "full-time" => Some("full-time"),
    "part-time" => Some("part-time"),
    "contract" => Some("contract"),
    "internship" => Some("intern"),
    _ => None,
  }
}

/// How one applies: on the company's site (`offsite_apply`) or with
/// LinkedIn's Easy Apply, and `apply_url`: the company's application
/// link where the page gives it, else the job's page.
fn apply(extra: &mut Extra, html: &str, url: &str) {
  let offsite = if html.contains("apply-link-offsite") {
    Some(true)
  } else if html.contains("Easy Apply") || html.contains("apply-link-onsite") {
    Some(false)
  } else {
    None
  };
  if let Some(offsite) = offsite {
    extra.insert("offsite_apply".into(), json!(offsite));
  }
  if html.contains("No longer accepting applications") {
    extra.insert("closed".into(), json!(true));
  }
  let link = APPLY_URL
    .captures(html)
    .map(|c| attr(&c[1]))
    .map(|link| external(&link).unwrap_or(link));
  extra.insert("apply_url".into(), json!(link.as_deref().unwrap_or(url)));
}

/// The company's link inside LinkedIn's `externalApply/<id>?url=…`.
fn external(link: &str) -> Option<String> {
  let url = url::Url::parse(link).ok()?;
  let (_, target) = url.query_pairs().find(|(k, _)| k == "url")?;
  target.starts_with("http").then(|| target.into_owned())
}
