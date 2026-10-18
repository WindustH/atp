//! Posts, users and categories as arguments.
//!
//! - posts: `https://eleduck.com/posts/<id>` (what lists print; a `#…`
//!   part is ignored), the job pages `https://eleduck.com/tposts/<id>`,
//!   `posts/<id>`, or the id (`GzfAwN`);
//! - comments: the id `comments` lists;
//! - users: `https://eleduck.com/users/<id>`, `users/<id>`, `@<id>`, or the
//!   id (`n0h5Ge`);
//! - categories: `https://eleduck.com/categories/<id>`, or the number.

use atp_core::{Error, Result};
use url::Url;

/// The path of a link (or of `posts/<id>` as typed), without the slashes
/// around it, the query and the `#` part.
fn path_of(arg: &str) -> String {
  let arg = arg.trim();
  match Url::parse(arg) {
    Ok(url) => url.path().trim_matches('/').to_owned(),
    Err(_) => arg
      .split(['#', '?'])
      .next()
      .unwrap_or(arg)
      .trim_matches('/')
      .to_owned(),
  }
}

/// Eleduck's ids are short hashids: letters and digits.
fn is_id(text: &str) -> bool {
  (3..=16).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn post(arg: &str) -> Result<String> {
  let path = path_of(arg);
  let id = ["posts/", "tposts/"]
    .iter()
    .find_map(|prefix| path.strip_prefix(prefix))
    .unwrap_or(&path);
  let id = id.split('/').next().unwrap_or(id);
  if is_id(id) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!(
      "not an Eleduck post: {arg} (expected a /posts/ link or an id)"
    )))
  }
}

pub fn user(arg: &str) -> Result<String> {
  let path = path_of(arg);
  let id = path.strip_prefix("users/").unwrap_or(&path);
  let id = id.split('/').next().unwrap_or(id).trim_start_matches('@');
  if is_id(id) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!(
      "not an Eleduck user: {arg} (expected a /users/ link or an id)"
    )))
  }
}

pub fn category(arg: &str) -> Result<u64> {
  let path = path_of(arg);
  let id = path.strip_prefix("categories/").unwrap_or(&path);
  id.parse().map_err(|_| {
    Error::input(format!(
      "not an Eleduck category: {arg} (expected a /categories/ link or its number; see `categories`)"
    ))
  })
}

/// A comment id as `comments` lists it.
pub fn comment(arg: &str) -> Result<String> {
  let id = arg.trim().rsplit("comment-").next().unwrap_or_default();
  if is_id(id) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!("not an Eleduck comment id: {arg}")))
  }
}
