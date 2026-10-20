//! Threads, replies and boards as arguments, in the forms the site and
//! the app give them:
//!
//! - threads: `https://bbs.hupu.com/<tid>.html` (what lists print), a page
//!   of it (`<tid>-2.html`), the mobile and share links
//!   (`m.hupu.com/bbs/<tid>.html`), text pasted from the app's share
//!   button with such a link in it, or the number;
//! - replies: the id `comments` prints;
//! - boards: their page (`https://bbs.hupu.com/history`, any list of it:
//!   `history-postdate-2`), the part after the host (`history`, `502`),
//!   or a name looked up in [`crate::boards`].

use std::sync::LazyLock;

use atp_core::{Error, Result};
use regex::Regex;

static LINK: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"https?://[A-Za-z0-9.-]*hupu\.com/[^\s'\x22<>]*").unwrap());
static THREAD: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(\d{6,12})(?:[-_]\d+)?(?:\.html?)?$").unwrap());
static LIST: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^([A-Za-z0-9][A-Za-z0-9-]*?)(?:-(?:postdate|hot))?(?:-\d+)?$").unwrap()
});

/// The link in an argument: the argument itself, or the first Hupu link in
/// text pasted from the app.
fn link(arg: &str) -> Option<url::Url> {
  let arg = arg.trim();
  let found = LINK.find(arg).map(|m| m.as_str())?;
  url::Url::parse(found).ok()
}

/// A thread's number from its link, the text around it or the number.
pub fn thread(arg: &str) -> Result<String> {
  let trimmed = arg.trim();
  if let Some(caps) = THREAD.captures(trimmed) {
    return Ok(caps[1].to_owned());
  }
  let found = link(trimmed).and_then(|url| {
    let from_path = url
      .path_segments()?
      .rev()
      .find_map(|segment| THREAD.captures(segment).map(|c| c[1].to_owned()));
    from_path.or_else(|| {
      url
        .query_pairs()
        .find(|(k, _)| k == "tid")
        .map(|(_, v)| v.into_owned())
        .filter(|v| v.bytes().all(|b| b.is_ascii_digit()) && !v.is_empty())
    })
  });
  found.ok_or_else(|| {
    Error::input(format!(
      "not a Hupu thread: {arg} (expected a bbs.hupu.com/<tid>.html link or the number)"
    ))
  })
}

/// A reply's number (`pid`), as `comments` prints it.
pub fn reply(arg: &str) -> Result<String> {
  let arg = arg.trim();
  let pid = link(arg)
    .and_then(|url| {
      url
        .query_pairs()
        .find(|(k, _)| k == "pid")
        .map(|(_, v)| v.into_owned())
    })
    .unwrap_or_else(|| arg.to_owned());
  if !pid.is_empty() && pid.len() <= 20 && pid.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(pid);
  }
  Err(Error::input(format!(
    "not a Hupu reply: {arg} (expected the id `comments` prints)"
  )))
}

/// A board's place on the site (`history`, `502`, `topic-daily`) from its
/// link or that part of it; `None` for anything else (a name).
pub fn board(arg: &str) -> Option<String> {
  let arg = arg.trim();
  let path = match link(arg) {
    Some(url) if url.host_str() == Some("bbs.hupu.com") => url.path().trim_matches('/').to_owned(),
    Some(_) => return None,
    None => arg.trim_matches('/').to_owned(),
  };
  if path.is_empty() || path.contains('/') || path.ends_with(".html") {
    return None;
  }
  LIST.captures(&path).map(|c| c[1].to_owned())
}
