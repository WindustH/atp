//! Users and comments. A profile keeps what its public page shows: name,
//! picture, self-introduction, website and social links, how many it
//! follows, Premium and official marks; the region, age, birthday,
//! gender and job the site may hold are left out.

use atp_core::text::html_to_text;
use atp_core::{Comment, User, Value, ValueExt, json};

use super::user_url;

/// A profile (`/ajax/user/<id>?full=1`).
pub fn user(v: &Value) -> User {
  let id = v.str("userId").unwrap_or_default();
  let mut user = User {
    name: v.str("name").unwrap_or_else(|| id.clone()),
    url: Some(user_url(&id)),
    avatar: v.first_str(&["imageBig", "image"]),
    bio: v
      .str("comment")
      .filter(|c| !c.trim().is_empty())
      .or_else(|| v.str("commentHtml").map(|h| html_to_text(&h))),
    followed: v.bool("isFollowed"),
    raw: Some(v.clone()),
    id,
    ..User::default()
  };
  user.stats.following = v.count("following");
  if let Some(webpage) = v.str("webpage") {
    user.extra.insert("website".into(), json!(webpage));
  }
  let social: serde_json::Map<String, Value> = v
    .at("social")
    .as_object()
    .into_iter()
    .flatten()
    .filter_map(|(site, link)| Some((site.clone(), json!(link.str("url")?))))
    .collect();
  if !social.is_empty() {
    user.extra.insert("social".into(), Value::Object(social));
  }
  for (key, path) in [("premium", "premium"), ("official", "official")] {
    if v.bool(path) == Some(true) {
      user.extra.insert(key.into(), json!(true));
    }
  }
  if v.bool("followedBack") == Some(true) {
    user.extra.insert("mutual".into(), json!(true));
  }
  user
}

/// A row of a following list (`/ajax/user/<id>/following`).
pub fn followed(v: &Value) -> User {
  let id = v.str("userId").unwrap_or_default();
  let mut user = User {
    name: v.str("userName").unwrap_or_else(|| id.clone()),
    url: Some(user_url(&id)),
    avatar: v.str("profileImageUrl"),
    bio: v.str("userComment").filter(|c| !c.trim().is_empty()),
    followed: v.bool("following"),
    raw: Some(v.clone()),
    id,
    ..User::default()
  };
  if v.bool("isMypixiv") == Some(true) {
    user.extra.insert("mypixiv".into(), json!(true));
  }
  if v.bool("followed") == Some(true) {
    user.extra.insert("mutual".into(), json!(true));
  }
  user
}

/// A comment of a work, or a reply to one. Stamps (pictures sent instead
/// of words) read `[stamp <id>]`.
pub fn comment(v: &Value) -> Comment {
  let uid = v.str("userId").filter(|id| id != "0");
  let author = uid.map(|id| User {
    name: v.str("userName").unwrap_or_else(|| id.clone()),
    url: Some(user_url(&id)),
    avatar: v.str("img"),
    id,
    ..User::default()
  });
  let text = v
    .str("comment")
    .filter(|c| !c.trim().is_empty())
    .or_else(|| v.str("stampId").map(|s| format!("[stamp {s}]")))
    .unwrap_or_default();
  let mut comment = Comment {
    id: v.str("id").unwrap_or_default(),
    author,
    text,
    created_at: v.str("commentDate").and_then(|d| tokyo(&d)),
    reply_to: v.str("replyToUserName"),
    raw: Some(v.clone()),
    ..Comment::default()
  };
  if v.bool("hasReplies") == Some(true) {
    comment.extra.insert("has_replies".into(), json!(true));
  }
  if let Some(parent) = v.str("commentParentId") {
    comment.extra.insert("parent_id".into(), json!(parent));
  }
  comment
}

/// A Japan (UTC+9) wall-clock time as comments give it
/// (`2024-05-06 21:37`).
fn tokyo(local: &str) -> Option<jiff::Timestamp> {
  let with_zone = format!("{} +0900", local.trim());
  ["%Y-%m-%d %H:%M:%S %z", "%Y-%m-%d %H:%M %z"]
    .iter()
    .find_map(|fmt| {
      jiff::fmt::strtime::parse(fmt, &with_zone)
        .ok()?
        .to_timestamp()
        .ok()
    })
}
