//! Jobs, posts, users, companies and comments as arguments.
//!
//! - jobs: `https://www.nowcoder.com/jobs/detail/<id>` (what lists print),
//!   `jobs/detail/<id>`, or the number (`467145`);
//! - posts: moments (动态) `https://www.nowcoder.com/feed/main/detail/<uuid>`
//!   or the 32 hex digits of the uuid; long posts (帖子)
//!   `https://www.nowcoder.com/discuss/<id>`, or their number when it has
//!   15 digits or more (a shorter number is a job);
//! - users: `https://www.nowcoder.com/users/<id>`, `users/<id>`, or the
//!   number;
//! - companies: `https://www.nowcoder.com/jobs/school/schedule/<id>` (what
//!   `schedule` prints), `https://www.nowcoder.com/enterprise/<id>`, or
//!   the number;
//! - comments: the id `comments` lists.

use atp_core::{Error, Result};
use url::Url;

use crate::api::WWW;

/// What `read`, `comments`, `like` … take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostRef {
  /// A job, by its number.
  Job(String),
  /// A moment (动态), by its uuid.
  Moment(String),
  /// A long post (帖子), by its number.
  Content(String),
}

impl PostRef {
  pub fn url(&self) -> String {
    match self {
      Self::Job(id) => job_url(id),
      Self::Moment(uuid) => moment_url(uuid),
      Self::Content(id) => content_url(id),
    }
  }
}

pub fn job_url(id: &str) -> String {
  format!("{WWW}/jobs/detail/{id}")
}

pub fn moment_url(uuid: &str) -> String {
  format!("{WWW}/feed/main/detail/{uuid}")
}

pub fn content_url(id: &str) -> String {
  format!("{WWW}/discuss/{id}")
}

pub fn user_url(id: &str) -> String {
  format!("{WWW}/users/{id}")
}

pub fn company_url(id: &str) -> String {
  format!("{WWW}/enterprise/{id}")
}

pub fn schedule_url(id: &str) -> String {
  format!("{WWW}/jobs/school/schedule/{id}")
}

/// The path of a link (or of `jobs/detail/<id>` as typed), without the
/// slashes around it, the query and the `#` part.
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

fn digits(text: &str) -> bool {
  (1..=20).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit())
}

fn is_uuid(text: &str) -> bool {
  text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Digits of a long post: its numbers have 15 digits or more.
const CONTENT_DIGITS: usize = 15;

pub fn post(arg: &str) -> Result<PostRef> {
  let path = path_of(arg);
  let parts: Vec<&str> = path.split('/').collect();
  let parsed = match parts.as_slice() {
    ["jobs", "detail", id, ..] if digits(id) => Some(PostRef::Job(id.to_string())),
    ["feed", "main", "detail", uuid, ..] if is_uuid(uuid) => {
      Some(PostRef::Moment(uuid.to_lowercase()))
    }
    ["discuss", id, ..] if digits(id) => Some(PostRef::Content(id.to_string())),
    [uuid] if is_uuid(uuid) => Some(PostRef::Moment(uuid.to_lowercase())),
    [id] if digits(id) && id.len() >= CONTENT_DIGITS => Some(PostRef::Content(id.to_string())),
    [id] if digits(id) => Some(PostRef::Job(id.to_string())),
    _ => None,
  };
  parsed.ok_or_else(|| {
    Error::input(format!(
      "not a Nowcoder job or post: {arg} (expected a /jobs/detail/, /feed/main/detail/ or /discuss/ link, a job number or a post uuid)"
    ))
  })
}

/// A job, by link or number.
pub fn job(arg: &str) -> Result<String> {
  match post(arg)? {
    PostRef::Job(id) => Ok(id),
    _ => Err(Error::input(format!(
      "not a Nowcoder job: {arg} (expected a /jobs/detail/ link or a job number)"
    ))),
  }
}

pub fn user(arg: &str) -> Result<String> {
  let path = path_of(arg);
  let id = path.strip_prefix("users/").unwrap_or(&path);
  let id = id.split('/').next().unwrap_or(id);
  if digits(id) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!(
      "not a Nowcoder user: {arg} (expected a /users/ link or the user's number)"
    )))
  }
}

pub fn company(arg: &str) -> Result<String> {
  let path = path_of(arg);
  let id = ["jobs/school/schedule/", "enterprise/"]
    .iter()
    .find_map(|prefix| path.strip_prefix(prefix))
    .unwrap_or(&path);
  let id = id.split('/').next().unwrap_or(id);
  if digits(id) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!(
      "not a Nowcoder company: {arg} (expected a /jobs/school/schedule/ or /enterprise/ link or its number)"
    )))
  }
}

/// A comment id as `comments` lists it.
pub fn comment(arg: &str) -> Result<String> {
  let id = arg.trim();
  if digits(id) {
    Ok(id.to_owned())
  } else {
    Err(Error::input(format!("not a Nowcoder comment id: {arg}")))
  }
}
