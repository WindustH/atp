//! Articles, comments, users and tags as arguments. Links are read by
//! their path, so links of the community in use work (`dev.to` by
//! default).
//!
//! - articles: the id (`2045821`), a link (`https://dev.to/ben/my-post-1k2j`)
//!   or `USER/SLUG`;
//! - comments: the id code (`2n5ko`) or a link (`https://dev.to/ben/comment/2n5ko`);
//! - users: the username, `@name`, a profile link, or the numeric id;
//! - tags: `rust`, `#rust` or a `/t/rust` link.

use atp_core::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArticleRef {
  Id(u64),
  /// `USER/SLUG`, as in the article's link.
  Path {
    user: String,
    slug: String,
  },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRef {
  Id(u64),
  Name(String),
}

/// The path segments of a link, or of the argument itself.
fn segments(arg: &str) -> Vec<String> {
  let arg = arg.trim();
  let path = match url::Url::parse(arg) {
    Ok(url) if url.has_host() => url.path().to_owned(),
    _ => arg.split(['?', '#']).next().unwrap_or_default().to_owned(),
  };
  path
    .split('/')
    .filter(|s| !s.is_empty())
    .map(str::to_owned)
    .collect()
}

fn name(s: &str) -> bool {
  !s.is_empty()
    && s
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn article(arg: &str) -> Result<ArticleRef> {
  if let Ok(id) = arg.trim().parse() {
    return Ok(ArticleRef::Id(id));
  }
  match segments(arg).as_slice() {
    [user, slug] if name(user) && name(slug) && user != "t" => Ok(ArticleRef::Path {
      user: user.clone(),
      slug: slug.clone(),
    }),
    _ => Err(Error::input(format!(
      "not a Forem article: {arg} (expected an id, a link or USER/SLUG)"
    ))),
  }
}

pub fn comment(arg: &str) -> Result<String> {
  let parts = segments(arg);
  let code = match parts.as_slice() {
    [.., marker, code] if marker == "comment" => code.clone(),
    [code] => code.clone(),
    _ => String::new(),
  };
  if name(&code) {
    Ok(code)
  } else {
    Err(Error::input(format!(
      "not a Forem comment: {arg} (expected its id code or link)"
    )))
  }
}

pub fn user(arg: &str) -> Result<UserRef> {
  let arg = arg.trim().trim_start_matches('@');
  if let Ok(id) = arg.parse() {
    return Ok(UserRef::Id(id));
  }
  match segments(arg).as_slice() {
    [user, ..] if name(user) => Ok(UserRef::Name(user.clone())),
    _ => Err(Error::input(format!(
      "not a Forem user: {arg} (expected a username or a profile link)"
    ))),
  }
}

pub fn tag(arg: &str) -> Result<String> {
  let arg = arg.trim().trim_start_matches('#');
  let tag = match segments(arg).as_slice() {
    [.., marker, tag] if marker == "t" => tag.clone(),
    [tag] => tag.clone(),
    _ => String::new(),
  };
  if name(&tag) {
    Ok(tag.to_lowercase())
  } else {
    Err(Error::input(format!(
      "not a Forem tag: {arg} (expected a name or a /t/ link)"
    )))
  }
}
