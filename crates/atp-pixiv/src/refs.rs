//! Works and users as arguments, in the forms the site and the app give
//! them:
//!
//! - artworks (illustrations, manga, ugoira): `https://www.pixiv.net/artworks/<id>`
//!   (what lists print; with a language, `/en/artworks/<id>`), the short
//!   `pixiv.net/i/<id>`, the old `member_illust.php?illust_id=<id>`, an
//!   image link of `i.pximg.net` (`<id>_p0.png`), text pasted from the
//!   app's share button with such a link in it, or the number;
//! - novels: `https://www.pixiv.net/novel/show.php?id=<id>` or `novel/<id>`;
//! - users: `https://www.pixiv.net/users/<id>` (`/en/users/<id>`), the old
//!   `member.php?id=<id>`, or the number.

use std::sync::LazyLock;

use atp_core::{Error, Result};
use regex::Regex;

static LINK: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"https?://(?:[A-Za-z0-9-]+\.)*(?:pixiv\.net|pximg\.net)/[^\s'\x22<>]*").unwrap()
});
static IMAGE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"/(\d+)_(?:p\d+|ugoira\d*|master\d*|square\d*)").unwrap());
static NOVEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^novel[/:](\d+)$").unwrap());

/// A work of the site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkRef {
  /// An illustration, manga or ugoira (they share their numbers).
  Artwork(String),
  Novel(String),
}

impl WorkRef {
  pub fn id(&self) -> &str {
    match self {
      Self::Artwork(id) | Self::Novel(id) => id,
    }
  }
}

fn numeric(s: &str) -> bool {
  !s.is_empty() && s.len() <= 12 && s.bytes().all(|b| b.is_ascii_digit())
}

/// The link in an argument: the argument itself, or the first pixiv link
/// in text pasted from the app.
fn link(arg: &str) -> Option<url::Url> {
  LINK
    .find(arg.trim())
    .and_then(|m| url::Url::parse(m.as_str()).ok())
}

/// The path of a link without its language (`/en/artworks/1` →
/// [`artworks`, `1`]).
fn segments(url: &url::Url) -> Vec<String> {
  let mut parts: Vec<String> = url
    .path_segments()
    .map(|s| s.filter(|p| !p.is_empty()).map(str::to_owned).collect())
    .unwrap_or_default();
  if parts
    .first()
    .is_some_and(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_lowercase()))
  {
    parts.remove(0);
  }
  parts
}

fn query(url: &url::Url, key: &str) -> Option<String> {
  url
    .query_pairs()
    .find(|(k, _)| k == key)
    .map(|(_, v)| v.into_owned())
    .filter(|v| numeric(v))
}

/// A work from its link, `novel/<id>` or the number of an artwork.
pub fn work(arg: &str) -> Result<WorkRef> {
  let trimmed = arg.trim();
  if numeric(trimmed) {
    return Ok(WorkRef::Artwork(trimmed.to_owned()));
  }
  if let Some(caps) = NOVEL.captures(trimmed) {
    return Ok(WorkRef::Novel(caps[1].to_owned()));
  }
  let found = link(trimmed).and_then(|url| work_of(&url));
  found.ok_or_else(|| {
    let what = if user(trimmed).is_ok() {
      " (this is a user: try `user` or `user-posts`)"
    } else {
      ""
    };
    Error::input(format!(
      "not a pixiv work: {arg}{what}; expected an artwork link (pixiv.net/artworks/<id>), a novel link (pixiv.net/novel/show.php?id=<id>) or an artwork's number"
    ))
  })
}

fn work_of(url: &url::Url) -> Option<WorkRef> {
  if url.host_str()?.ends_with("pximg.net") {
    let id = IMAGE.captures(url.path())?[1].to_owned();
    return Some(WorkRef::Artwork(id));
  }
  let parts = segments(url);
  match parts
    .iter()
    .map(String::as_str)
    .collect::<Vec<_>>()
    .as_slice()
  {
    ["artworks" | "i", id, ..] if numeric(id) => Some(WorkRef::Artwork((*id).to_owned())),
    ["member_illust.php"] => query(url, "illust_id").map(WorkRef::Artwork),
    ["novel", "show.php"] => query(url, "id").map(WorkRef::Novel),
    ["novel", id] if numeric(id) => Some(WorkRef::Novel((*id).to_owned())),
    _ => None,
  }
}

/// A user's number from their link or the number.
pub fn user(arg: &str) -> Result<String> {
  let trimmed = arg.trim();
  if numeric(trimmed) {
    return Ok(trimmed.to_owned());
  }
  let found = link(trimmed).and_then(|url| {
    let parts = segments(&url);
    match parts
      .iter()
      .map(String::as_str)
      .collect::<Vec<_>>()
      .as_slice()
    {
      ["users", id, ..] if numeric(id) => Some((*id).to_owned()),
      ["member.php" | "member_illust.php"] => query(&url, "id"),
      _ => None,
    }
  });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a pixiv user: {arg}; expected a pixiv.net/users/<id> link or the number"
    ))
  })
}
