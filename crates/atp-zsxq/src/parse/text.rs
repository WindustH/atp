//! The text of topics and comments, and the API's times.
//!
//! Texts are plain with tags for what the editor inserts:
//! `<e type="mention" uid="…" title="%40name" />`, `<e type="hashtag"
//! hid="…" title="%23tag%23" />`, `<e type="web" href="…" title="…" />`,
//! `<e type="text_bold" title="…" />` …, their values URL-encoded.
//! Times are `2026-10-20T12:00:00.000+0800`; the listings page by them
//! (`end_time`, `begin_time`), a millisecond past the last item.

use std::sync::LazyLock;

use atp_core::text::decode_uri_component;
use jiff::{SignedDuration, Timestamp, tz};
use regex::Regex;

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<e\s([^>]*?)/?>").unwrap());
static ATTR: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"([A-Za-z_]+)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap());

/// The value of one attribute of a tag, decoded.
fn attr(attrs: &str, name: &str) -> Option<String> {
  ATTR
    .captures_iter(attrs)
    .find(|c| &c[1] == name)
    .and_then(|c| c.get(2).or_else(|| c.get(3)))
    .map(|m| decode_uri_component(m.as_str()).trim().to_owned())
    .filter(|s| !s.is_empty())
}

/// What a tag reads as: `@name`, `#tag#`, a link with its title …
fn tag_text(attrs: &str) -> String {
  let title = attr(attrs, "title");
  match attr(attrs, "type").as_deref() {
    Some("web") => {
      let href = attr(attrs, "href");
      match (title, href) {
        (Some(title), Some(href)) if title != href => format!("{title} ({href})"),
        (_, Some(href)) => href,
        (title, None) => title.unwrap_or_default(),
      }
    }
    _ => title.unwrap_or_default(),
  }
}

/// A text with its tags read out.
pub fn plain(raw: &str) -> String {
  TAG
    .replace_all(raw, |caps: &regex::Captures| tag_text(&caps[1]))
    .trim()
    .to_owned()
}

/// The hashtags of a text, without their `#`.
pub fn hashtags(raw: &str) -> Vec<String> {
  let mut tags: Vec<String> = Vec::new();
  for caps in TAG.captures_iter(raw) {
    if attr(&caps[1], "type").as_deref() != Some("hashtag") {
      continue;
    }
    if let Some(title) = attr(&caps[1], "title") {
      let tag = title.trim_matches('#').trim().to_owned();
      if !tag.is_empty() && !tags.contains(&tag) {
        tags.push(tag);
      }
    }
  }
  tags
}

/// A time of the API (`2026-10-20T12:00:00.000+0800`).
pub fn time(s: &str) -> Option<Timestamp> {
  let s = s.trim();
  s.parse::<Timestamp>().ok().or_else(|| {
    jiff::fmt::strtime::parse("%Y-%m-%dT%H:%M:%S%.f%z", s)
      .ok()?
      .to_timestamp()
      .ok()
  })
}

/// A time as the API takes it in `end_time` / `begin_time`: Beijing time
/// with milliseconds.
pub fn api_time(ts: Timestamp) -> String {
  ts.to_zoned(tz::TimeZone::fixed(tz::offset(8)))
    .strftime("%Y-%m-%dT%H:%M:%S%.3f%z")
    .to_string()
}

/// `time` moved by `ms` milliseconds, in the API's form: the cursor past
/// an item (`-1` for newest-first listings, `1` for oldest-first).
pub fn shifted(time_text: &str, ms: i64) -> Option<String> {
  let ts = time(time_text)?;
  ts.checked_add(SignedDuration::from_millis(ms))
    .ok()
    .map(api_time)
}
