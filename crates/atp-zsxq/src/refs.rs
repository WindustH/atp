//! Topics, groups, users and comments as arguments, in the forms the site
//! gives them:
//!
//! - topics: `https://wx.zsxq.com/topic/<id>` (what lists print), a link
//!   with the topic inside a group (`…/group/<gid>/topic/<id>`) or with
//!   `topic_id=` in it, or the number;
//! - groups: `https://wx.zsxq.com/group/<gid>` (what `groups` prints), the
//!   public page (`https://public.zsxq.com/groups/<gid>.html`), a link with
//!   `group_id=`, or the number;
//! - users: `https://wx.zsxq.com/footprint/<uid>` (their page), a link with
//!   `user_id=`, or the number;
//! - comments: the id `comments` prints.
//!
//! Text pasted around a link (a share message) is fine: the first zsxq.com
//! link in it counts.

use std::sync::LazyLock;

use atp_core::{Error, Result};
use regex::Regex;
use url::Url;

static LINK: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"https?://[A-Za-z0-9.-]*zsxq\.com/[^\s'\x22<>]*").unwrap());

/// The link in an argument: the argument itself, or the first zsxq.com
/// link in text around it.
fn link(arg: &str) -> Option<Url> {
  let found = LINK.find(arg.trim())?;
  Url::parse(found.as_str()).ok()
}

/// A number as the site's ids are: digits only.
fn number(s: &str) -> Option<String> {
  let s = s.trim().trim_end_matches(".html");
  (!s.is_empty() && s.len() <= 24 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.to_owned())
}

/// The number after one of `names` in a link's path (`/topic/<id>`).
fn after(url: &Url, names: &[&str]) -> Option<String> {
  let segments: Vec<&str> = url.path_segments()?.collect();
  segments
    .windows(2)
    .rev()
    .find(|pair| names.contains(&pair[0]))
    .and_then(|pair| number(pair[1]))
}

fn query(url: &Url, key: &str) -> Option<String> {
  url
    .query_pairs()
    .find(|(k, _)| k == key)
    .and_then(|(_, v)| number(&v))
}

/// The number of an argument: itself, or what `find` reads from its link.
fn parse(
  arg: &str,
  what: &str,
  forms: &str,
  find: impl Fn(&Url) -> Option<String>,
) -> Result<String> {
  if let Some(id) = number(arg) {
    return Ok(id);
  }
  link(arg).as_ref().and_then(find).ok_or_else(|| {
    Error::input(format!(
      "not a 知识星球 {what}: {arg} (expected {forms} or the number)"
    ))
  })
}

pub fn topic(arg: &str) -> Result<String> {
  parse(arg, "topic", "a wx.zsxq.com/topic/<id> link", |url| {
    after(url, &["topic", "topics", "topic_detail"]).or_else(|| query(url, "topic_id"))
  })
}

pub fn group(arg: &str) -> Result<String> {
  parse(arg, "group", "a wx.zsxq.com/group/<id> link", |url| {
    after(url, &["group", "groups"]).or_else(|| query(url, "group_id"))
  })
}

pub fn user(arg: &str) -> Result<String> {
  parse(arg, "user", "a wx.zsxq.com/footprint/<id> link", |url| {
    after(url, &["footprint", "users", "user"]).or_else(|| query(url, "user_id"))
  })
}

pub fn comment(arg: &str) -> Result<String> {
  number(arg).ok_or_else(|| {
    Error::input(format!(
      "not a 知识星球 comment: {arg} (expected the id `comments` prints)"
    ))
  })
}
