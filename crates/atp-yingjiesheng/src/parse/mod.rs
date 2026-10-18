//! Yingjiesheng pages → the shared model: talks (宣讲会) as posts, from
//! the rows of the talk list and from talk pages, with their companies as
//! users; a talk's start and end times from the mobile site's page
//! ([`nuxt`]), and its city and description there when the older page
//! leaves them empty.
//!
//! The older pages show a talk's time of day as a picture
//! (`/xjhimg/<id>_time.png`), which ATP does not read: a list gives the
//! day, a talk page the times from the mobile site.

mod nuxt;

pub use nuxt::fill;

use std::sync::LazyLock;

use atp_core::model::Extra;
use atp_core::text::{date_only, html_to_text};
use atp_core::{Post, User, json};
use regex::{Captures, Regex};

use crate::refs;

fn pattern(text: &str) -> Regex {
  Regex::new(text).expect("valid pattern")
}

static ROW_ID: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"id="r_comments_e(\d+)"><a [^>]*>([^<]*)</a>"#));
static CITY: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"xuanjianghui_city_(\d+)\.html"[^>]*>([^<]+)</a>"#));
static SCHOOL: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"xuanjianghui_school_(\d+)\.html"[^>]*>([^<]+)</a>"#));
static DAY: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"<span class="i">(\d{4}-\d{2}-\d{2})<br>"#));
static VENUE: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"<td width="290"><span class="i">([^<]*)</span>"#));
static LAST_PAGE: LazyLock<Regex> = LazyLock::new(|| pattern(r#"page=(\d+)[^"]*" title="末页""#));

/// Text of a piece of a page: entities decoded, spaces trimmed.
fn clean(html: &str) -> String {
  html_to_text(html).trim().to_owned()
}

/// `id` and name of the first link `re` finds.
fn link(re: &Regex, html: &str) -> Option<(String, String)> {
  let caps = re.captures(html)?;
  Some((caps[1].to_owned(), clean(&caps[2])))
}

/// The keys of where a talk is: city, school, venue (`venue`), with the
/// site's numbers of the city and school.
fn place(extra: &mut Extra, city: Option<(String, String)>, school: Option<(String, String)>) {
  for (key, found) in [("city", city), ("school", school)] {
    if let Some((id, name)) = found {
      extra.insert(key.into(), json!(name));
      extra.insert(format!("{key}_id"), json!(id));
    }
  }
}

/// The company of a talk; its number when the page links its talks.
fn company(id: Option<String>, name: String) -> User {
  User {
    url: id.as_deref().map(refs::company_url),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  }
}

/// A row of the talk list: company, day, city, school and venue.
fn row(html: &str) -> Option<Post> {
  let caps: Captures = ROW_ID.captures(html)?;
  let id = caps[1].to_owned();
  let name = clean(&caps[2]);
  let school = link(&SCHOOL, html);
  let venue = VENUE.captures(html).map(|c| clean(&c[1]));
  let day = DAY.captures(html).map(|c| c[1].to_owned());
  let title = [school.as_ref().map(|(_, s)| s.clone()), venue.clone()]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
  let mut post = Post {
    url: Some(refs::talk_url(&id)),
    kind: "talk".into(),
    title: (!title.is_empty()).then_some(title),
    created_at: day
      .as_deref()
      .and_then(|d| d.parse().ok())
      .and_then(date_only),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  extra.insert("company".into(), json!(name));
  place(extra, link(&CITY, html), school);
  if let Some(venue) = venue.filter(|v| !v.is_empty()) {
    extra.insert("venue".into(), json!(venue));
  }
  if let Some(day) = day {
    extra.insert("date".into(), json!(day));
  }
  post.author = Some(company(None, name));
  Some(post)
}

/// The rows of a page of the talk list.
pub fn rows(html: &str) -> Vec<Post> {
  let list = html
    .split_once(r#"<div class="listul">"#)
    .map_or("", |(_, rest)| rest);
  list.split("<tr").filter_map(row).collect()
}

/// The last page the list's pager names (`尾页`), when it has one.
pub fn last_page(html: &str) -> Option<u64> {
  LAST_PAGE.captures(html)?[1].parse().ok()
}

static TITLE: LazyLock<Regex> = LazyLock::new(|| pattern(r"(?s)<h1>(.*?)</h1>"));
static COMPANY: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"xuanjianghui_cid_(\d+)\.html"[^>]*>([^<]+)<"#));
static FIELD: LazyLock<Regex> = LazyLock::new(|| {
  pattern(
    r#"(?s)<em class="ps">([^<：]+)：</em>\s*(?:<span class="i">(.*?)</span>|<a [^>]*>([^<]*)</a>)"#,
  )
});
static DETAIL: LazyLock<Regex> = LazyLock::new(|| {
  pattern(
    r#"(?s)<h4>宣讲会详情</h4></label>\s*<div class="inf">(.*?)</div>\s*</div>\s*(?:<div class="combox">|<script)"#,
  )
});
static SOURCE: LazyLock<Regex> =
  LazyLock::new(|| pattern(r#"id="frameUrl" type="hidden" value="([^"]*)""#));

/// The labelled fields under a talk page's title (地点, 备注 …), as text.
fn field(html: &str, label: &str) -> Option<String> {
  FIELD
    .captures_iter(html)
    .find(|caps| caps[1].trim() == label)
    .map(|caps| clean(caps.get(2).or(caps.get(3)).map_or("", |m| m.as_str())))
    .filter(|text| !text.is_empty())
}

/// A talk's page: title, company, city, school, venue, remark, the
/// description and the page it was taken from.
pub fn talk(html: &str, id: &str) -> Option<Post> {
  let head = html.split_once(r#"<div class="comtit">"#)?.1;
  let title = TITLE
    .captures(head)
    .map(|c| clean(&c[1].replace(['\r', '\n'], "")));
  let (company_id, name) = link(&COMPANY, head).map_or((None, None), |(id, n)| (Some(id), Some(n)));
  let mut post = Post {
    url: Some(refs::talk_url(id)),
    kind: "talk".into(),
    title,
    text: DETAIL
      .captures(html)
      .map(|c| clean(&c[1]))
      .filter(|t| !t.is_empty()),
    id: id.to_owned(),
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(name) = &name {
    extra.insert("company".into(), json!(name));
  }
  let head_end = head
    .split_once(r#"class="tborder""#)
    .map_or(head, |(h, _)| h);
  place(extra, link(&CITY, head_end), link(&SCHOOL, head_end));
  for (key, label) in [("venue", "地点"), ("remark", "备注")] {
    if let Some(value) = field(head_end, label) {
      extra.insert(key.into(), json!(value));
    }
  }
  let source = SOURCE.captures(html).map(|c| c[1].replace("&amp;", "&"));
  if let Some(source) = source.filter(|s| s.starts_with("http")) {
    extra.insert("source_url".into(), json!(source));
  }
  post.author = name.map(|name| company(company_id, name));
  post.title.as_ref()?;
  Some(post)
}
