//! Threads, replies, users and boards as arguments, in the forms the site's
//! own pages recognise:
//!
//! - threads: `https://www.1point3acres.com/bbs/thread-<tid>-<page>-<n>.html`
//!   (what lists print), `/home/thread/<tid>`, `/home/pins/<tid>`,
//!   `/thread/<tid>`, `forum.php?mod=viewthread&tid=<tid>`, a `findpost`
//!   link with `ptid=<tid>`, or the number;
//! - replies: the id `comments` prints, `/home/post/<pid>`, or a `findpost`
//!   link with `pid=<pid>`;
//! - users: `/home/u/<uid>`, `space-uid-<uid>.html`,
//!   `home.php?mod=space&uid=<uid>`, or the number;
//! - boards: `forum-<fid>-<page>.html`, `/home/forum/<fid>`, `/forum/<fid>`,
//!   `mod=forumdisplay&fid=<fid>`, or the number (names are looked up in
//!   [`crate::boards`]).

use atp_core::{Error, Result};
use url::Url;

/// Where links without a host are read from.
const FORUM: &str = "https://www.1point3acres.com/bbs/";

/// A link's path and query; links without a host (`thread-1-1-1.html`,
/// `forum.php?mod=viewthread&tid=1`) are read as the forum's.
fn parts(arg: &str) -> (String, Vec<(String, String)>) {
  let arg = arg.trim();
  let url = if arg.contains("://") {
    Url::parse(arg)
  } else if arg.contains("1point3acres.") {
    Url::parse(&format!("https://{arg}"))
  } else {
    Url::parse(FORUM).and_then(|base| base.join(arg))
  };
  match url {
    Ok(url) => {
      let query = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
      (url.path().trim_matches('/').to_owned(), query)
    }
    Err(_) => (arg.trim_matches('/').to_owned(), Vec::new()),
  }
}

fn number(text: &str) -> Option<u64> {
  let text = text.trim();
  if text.is_empty() || text.len() > 12 || !text.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  text.parse().ok().filter(|n| *n > 0)
}

/// The value of a query parameter that is a number.
fn param(query: &[(String, String)], key: &str) -> Option<u64> {
  query
    .iter()
    .find(|(k, _)| k == key)
    .and_then(|(_, v)| number(v))
}

/// The number after `prefix` in the last segment of a path:
/// `thread-123-1-1.html` with `thread-` gives 123.
fn after(path: &str, prefix: &str) -> Option<u64> {
  let last = path.rsplit('/').next()?;
  let rest = last.strip_prefix(prefix)?;
  number(rest.split(['-', '.']).next()?)
}

/// The number following one of these path segments: `home/thread/123`.
fn segment(path: &str, names: &[&str]) -> Option<u64> {
  let segments: Vec<&str> = path.split('/').collect();
  segments
    .windows(2)
    .find(|pair| names.contains(&pair[0]))
    .and_then(|pair| number(pair[1]))
}

pub fn thread(arg: &str) -> Result<u64> {
  let (path, query) = parts(arg);
  number(arg)
    .or_else(|| after(&path, "thread-"))
    .or_else(|| segment(&path, &["thread", "pins"]))
    .or_else(|| param(&query, "tid"))
    .or_else(|| param(&query, "ptid"))
    .ok_or_else(|| {
      Error::input(format!(
        "not a 1Point3Acres thread: {arg} (expected a thread-<tid>-1-1.html link or the number)"
      ))
    })
}

pub fn post(arg: &str) -> Result<u64> {
  let (path, query) = parts(arg);
  number(arg)
    .or_else(|| segment(&path, &["post"]))
    .or_else(|| param(&query, "pid"))
    .ok_or_else(|| {
      Error::input(format!(
        "not a 1Point3Acres reply: {arg} (expected the id `comments` prints)"
      ))
    })
}

pub fn user(arg: &str) -> Result<u64> {
  let (path, query) = parts(arg);
  if path.contains("space-username-") {
    return Err(Error::input(format!(
      "ATP finds 1Point3Acres users by their number (uid), not their name: {arg}"
    )));
  }
  number(arg.trim().trim_start_matches('@'))
    .or_else(|| after(&path, "space-uid-"))
    .or_else(|| segment(&path, &["u"]))
    .or_else(|| param(&query, "uid"))
    .ok_or_else(|| {
      Error::input(format!(
        "not a 1Point3Acres user: {arg} (expected a /home/u/<uid> link or the number)"
      ))
    })
}

/// A board's number from a link or the number; `None` for anything else
/// (a name, looked up among the boards).
pub fn board(arg: &str) -> Option<u64> {
  let (path, query) = parts(arg);
  number(arg)
    .or_else(|| after(&path, "forum-"))
    .or_else(|| segment(&path, &["forum"]))
    .or_else(|| param(&query, "fid"))
}
