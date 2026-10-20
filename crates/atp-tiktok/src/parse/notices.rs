//! Notices of the inbox (`/api/notice/multi/`): the payload key a notice
//! carries says what happened (`digg`, `comment`, `follow`, `at`), as on
//! Douyin; flatter shapes (`from_user`, `content`) are read too.

use super::*;

/// Payload keys and the kinds they stand for.
const PAYLOADS: &[(&str, &str)] = &[
  ("digg", "like"),
  ("comment", "comment"),
  ("follow", "follow"),
  ("at", "mention"),
];

pub fn notice(v: &Value) -> Notification {
  let (kind, body) = PAYLOADS
    .iter()
    .find(|(key, _)| v.at(key).is_object())
    .map_or(("other", v), |(key, kind)| (*kind, v.at(key)));
  let actor = [
    body.at("from_user.0"),
    body.at("from_users.0"),
    body.at("from_user"),
    body.at("user"),
    body.at("comment.user"),
    v.at("from_user"),
    v.at("fromUser"),
  ]
  .into_iter()
  .find(|u| u.is_object())
  .map(user_brief);
  let aweme = [body.at("aweme"), body.at("comment.aweme"), v.at("aweme")]
    .into_iter()
    .find(|a| a.is_object())
    .unwrap_or(&Value::Null);
  let url = aweme.str("aweme_id").map(|id| {
    let author = aweme.str("author.unique_id").unwrap_or_default();
    video_url(&author, &id, false)
  });
  Notification {
    id: v
      .first_str(&["nid_str", "nid", "notice_id", "id"])
      .unwrap_or_default(),
    kind: kind.into(),
    text: body
      .first_str(&["comment.text", "text", "content", "notice_content"])
      .or_else(|| v.first_str(&["content", "text"]))
      .unwrap_or_default(),
    actor,
    target: aweme.first_str(&["desc", "preview_title"]),
    url,
    created_at: v
      .first_str(&["create_time", "createTime", "timestamp"])
      .and_then(|t| t.parse().ok())
      .and_then(from_secs),
    unread: v.bool("has_read").map(|read| !read),
    raw: Some(v.clone()),
  }
}
