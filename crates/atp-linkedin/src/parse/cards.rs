//! The cards of the job search (`seeMoreJobPostings/search`): one `<li>`
//! per job with its URN (`data-entity-urn="urn:li:jobPosting:<id>"`), the
//! title, the company with its link and logo, the place, the pay where
//! LinkedIn shows it, its notes (`Actively Hiring`, `Be an early
//! applicant`) and the day it was listed.

use std::sync::LazyLock;

use atp_core::jobs::Salary;
use atp_core::{Post, json};
use regex::Regex;

use super::{
  attr, capture, day, employer, first_line, line, pay, pay_in, re, read_parts, remote, text_value,
};
use crate::refs;

static CARD: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)<li[^>]*>(.*?)</li>"));
static URN: LazyLock<Regex> = LazyLock::new(|| re(r#"data-entity-urn="urn:li:jobPosting:(\d+)""#));
static LINK: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<a[^>]*base-card__full-link[^>]*href="([^"]+)""#));
static TITLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h3 class="base-search-card__title">(.*?)</h3>"#));
static SUBTITLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h4 class="base-search-card__subtitle">(.*?)</h4>"#));
static HREF: LazyLock<Regex> = LazyLock::new(|| re(r#"href="([^"]+)""#));
static PLACE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="job-search-card__location">(.*?)</span>"#));
static SALARY: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="job-search-card__salary-info">(.*?)</span>"#));
static NOTE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="job-posting-benefits__text">(.*?)</span>"#));
static LISTED: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<time class="job-search-card__listdate[^"]*" datetime="([^"]+)""#));
static LOGO: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="artdeco-entity-image[^"]*"[^>]*data-delayed-url="([^"]+)""#));

/// The jobs of a page of the search, in its order.
pub fn cards(html: &str) -> Vec<Post> {
  CARD
    .captures_iter(html)
    .filter_map(|c| card(&c[1]))
    .collect()
}

/// Whether a page of the search has cards at all (the end of the results
/// is a page without any).
pub fn any(html: &str) -> bool {
  CARD.is_match(html)
}

/// The id of a card: its URN, else its link.
fn id(card: &str) -> Option<String> {
  URN.captures(card).map(|c| c[1].to_owned()).or_else(|| {
    let link = attr(&LINK.captures(card)?[1]);
    refs::job(&link).ok()
  })
}

/// One card.
fn card(html: &str) -> Option<Post> {
  let id = id(html)?;
  let title = capture(&TITLE, html)?;
  let subtitle = SUBTITLE.captures(html).map(|c| c[1].to_owned());
  let company = subtitle.as_deref().map(line).filter(|s| !s.is_empty());
  let link = subtitle
    .as_deref()
    .and_then(|s| HREF.captures(s))
    .map(|c| attr(&c[1]));
  let logo = LOGO.captures(html).map(|c| attr(&c[1]));
  let place = capture(&PLACE, html);
  let shown_pay = capture(&SALARY, html);
  let notes: Vec<String> = NOTE
    .captures_iter(html)
    .map(|c| first_line(&c[1]))
    .filter(|n| !n.is_empty())
    .collect();
  let listed = LISTED.captures(html).map(|c| c[1].to_owned());
  let mut post = Post {
    url: Some(refs::job_url(&id)),
    id,
    kind: "job".into(),
    created_at: listed.as_deref().and_then(day),
    author: company
      .as_deref()
      .and_then(|name| employer(name, link.as_deref(), logo)),
    tags: notes.clone(),
    raw: Some(read_parts(&[
      ("title", Some(json!(title))),
      ("company", text_value(&company)),
      ("company_link", text_value(&link)),
      ("location", text_value(&place)),
      ("salary", text_value(&shown_pay)),
      ("notes", (!notes.is_empty()).then(|| json!(notes))),
      ("listed", text_value(&listed)),
    ])),
    ..Post::default()
  };
  let salary = shown_pay.as_deref().and_then(pay);
  fill(&mut post, &title, company, place, salary);
  post.title = Some(title);
  Some(post)
}

/// The job keys a card tells: company, place, remote, pay (shown, or
/// named in the title).
fn fill(
  post: &mut Post,
  title: &str,
  company: Option<String>,
  place: Option<String>,
  salary: Option<Salary>,
) {
  let extra = &mut post.extra;
  if let Some(company) = company {
    extra.insert("company".into(), json!(company));
  }
  if remote(title, place.as_deref()) {
    extra.insert("remote".into(), json!(true));
  }
  if let Some(place) = place {
    extra.insert("city".into(), json!(place));
  }
  if let Some(salary) = salary.or_else(|| pay_in(title, None)) {
    salary.fill(extra);
  }
}
