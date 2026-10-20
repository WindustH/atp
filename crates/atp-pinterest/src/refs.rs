//! Pins, users and boards as arguments, in the forms the site and the app
//! give them, on any of Pinterest's country sites (`pinterest.com`,
//! `jp.pinterest.com`, `pinterest.co.uk` …):
//!
//! - pins: `https://www.pinterest.com/pin/<id>/` (what lists print), the
//!   short `https://pin.it/<code>` of the share button (followed to the
//!   pin), text pasted with such a link in it, or the number;
//! - users: `https://www.pinterest.com/<username>/`, the username, or the
//!   username with `@`;
//! - boards: `https://www.pinterest.com/<username>/<board>/`, the part
//!   after the host (`<username>/<board>`, what `collections` prints), or
//!   the board's number.

use std::sync::LazyLock;

use atp_core::{Error, Result};
use regex::Regex;

static LINK: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"https?://(?:[A-Za-z0-9-]+\.)*(?:pinterest\.[a-z]{2,3}(?:\.[a-z]{2})?|pin\.it)/[^\s'\x22<>]*",
  )
  .unwrap()
});
static PIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/pin/([A-Za-z0-9_-]+)").unwrap());
static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_.-]{1,64}$").unwrap());

/// The first parts of paths that are pages of the site, not users.
const RESERVED: &[&str] = &[
  "pin",
  "search",
  "ideas",
  "today",
  "settings",
  "_saved",
  "_created",
  "business",
  "resource",
  "explore",
  "categories",
  "topics",
  "shopping",
  "about",
  "login",
  "signup",
];

/// A pin as an argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinRef {
  Id(String),
  /// A `pin.it` link, to follow to the pin.
  Short(String),
}

/// A board as an argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardRef {
  Path { user: String, slug: String },
  Id(String),
}

fn link(arg: &str) -> Option<url::Url> {
  LINK
    .find(arg.trim())
    .and_then(|m| url::Url::parse(m.as_str()).ok())
}

/// The parts of a link's path, decoded.
fn parts(url: &url::Url) -> Vec<String> {
  url
    .path_segments()
    .map(|s| {
      s.filter(|p| !p.is_empty())
        .map(atp_core::text::decode_uri_component)
        .collect()
    })
    .unwrap_or_default()
}

pub fn pin(arg: &str) -> Result<PinRef> {
  let trimmed = arg.trim();
  if !trimmed.is_empty() && trimmed.len() <= 24 && trimmed.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(PinRef::Id(trimmed.to_owned()));
  }
  let found = link(trimmed).and_then(|url| {
    if url.host_str() == Some("pin.it") {
      return Some(PinRef::Short(url.to_string()));
    }
    PIN
      .captures(url.path())
      .map(|c| PinRef::Id(c[1].to_owned()))
  });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a Pinterest pin: {arg}; expected a pinterest.com/pin/<id>/ link, a pin.it link or the number"
    ))
  })
}

/// The id of a pin link (after a `pin.it` link was followed).
pub fn pin_of_link(url: &str) -> Option<String> {
  let url = url::Url::parse(url).ok()?;
  PIN.captures(url.path()).map(|c| c[1].to_owned())
}

pub fn user(arg: &str) -> Result<String> {
  let trimmed = arg.trim();
  let name = match link(trimmed) {
    Some(url) => parts(&url).into_iter().next().unwrap_or_default(),
    None => trimmed.trim_start_matches('@').trim_matches('/').to_owned(),
  };
  if NAME.is_match(&name) && !RESERVED.contains(&name.to_ascii_lowercase().as_str()) {
    return Ok(name);
  }
  Err(Error::input(format!(
    "not a Pinterest user: {arg}; expected a username or a pinterest.com/<username>/ link"
  )))
}

pub fn board(arg: &str) -> Result<BoardRef> {
  let trimmed = arg.trim();
  if !trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(BoardRef::Id(trimmed.to_owned()));
  }
  let path: Vec<String> = match link(trimmed) {
    Some(url) => parts(&url),
    None => trimmed
      .trim_matches('/')
      .split('/')
      .map(atp_core::text::decode_uri_component)
      .collect(),
  };
  match path.as_slice() {
    [user, slug, ..] if !RESERVED.contains(&user.to_ascii_lowercase().as_str()) => {
      Ok(BoardRef::Path {
        user: user.clone(),
        slug: slug.clone(),
      })
    }
    _ => Err(Error::input(format!(
      "not a Pinterest board: {arg}; expected a pinterest.com/<username>/<board>/ link, <username>/<board> or the board's number"
    ))),
  }
}
