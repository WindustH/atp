//! Your chats with recruiters, as the job seeker's chat page lists them:
//! a chat (`friendId`, the company, the job, the last message and its
//! time) is a collection, a message (`mid`, `time`, `from.uid`, `type`
//! and a `body`) is a post. Chats are named by company and job, never by
//! the recruiter; messages say only whether they came from you
//! (`direction`). Cards trading phone numbers, WeChat ids or résumés are
//! shown as such, without their contents.

use atp_core::chat::message_id;
use atp_core::{Collection, Media, Post, Value, ValueExt, json};

use super::{sanitize, text, time};
use crate::refs;

/// Keys of a chat that name or picture the recruiter.
const PERSON_KEYS: &[&str] = &["name", "title", "tinyUrl", "largeUrl", "certification"];

/// The chat without the recruiter's name, title and pictures.
fn without_person(v: &Value) -> Value {
  let mut v = sanitize(v);
  if let Some(map) = v.as_object_mut() {
    map.retain(|key, _| !PERSON_KEYS.contains(&key.as_str()));
  }
  v
}

/// A chat of the list, merged with its details (`getGeekFriendList`).
pub fn chat(v: &Value) -> Option<Collection> {
  let id = v.str("friendId").or_else(|| v.str("uid"))?;
  let company = text(v, "brandName");
  let job = text(v, "jobName");
  let name = match (&company, &job) {
    (Some(c), Some(j)) => format!("{c} · {j}"),
    (c, j) => c
      .clone()
      .or_else(|| j.clone())
      .unwrap_or_else(|| id.clone()),
  };
  let mut chat = Collection {
    kind: "dm".into(),
    name,
    raw: Some(without_person(v)),
    id,
    ..Collection::default()
  };
  let extra = &mut chat.extra;
  for (key, value) in [("company", company), ("job", job)] {
    if let Some(value) = value {
      extra.insert(key.into(), json!(value));
    }
  }
  if let Some(job) = text(v, "encryptJobId") {
    extra.insert("job_url".into(), json!(refs::job_url(&job)));
  }
  let last = v.at("lastMessageInfo");
  if let Some(at) = time(last, "msgTime").or_else(|| time(v, "updateTime")) {
    extra.insert("last_message_at".into(), json!(at));
  }
  if let Some(n) = v.u64("unreadMsgCount").filter(|n| *n > 0) {
    extra.insert("unread".into(), json!(n));
  }
  Some(chat)
}

/// Message types (`body.type`, as the chat page reads them): text, voice
/// and pictures; the others are cards (actions, résumés, jobs).
const TEXT: i64 = 1;
const VOICE: i64 = 2;
const IMAGE: i64 = 3;
/// The template of a text that is a phone number handed over.
const PHONE_TEMPLATE: i64 = 5;
/// Marks around phone numbers and WeChat ids in a text.
const HIDDEN: &[(&str, &str)] = &[("<phone>", "</phone>"), ("<copy>", "</copy>")];

/// A message's text, phone numbers and WeChat ids taken out.
fn plain(text: &str) -> String {
  let mut text = text.to_owned();
  for (open, close) in HIDDEN {
    while let Some(start) = text.find(open) {
      let end = text[start..]
        .find(close)
        .map_or(text.len(), |at| start + at + close.len());
      text.replace_range(start..end, "[contact]");
    }
  }
  text
}

/// A message of a chat with the recruiter `other` (their account number).
pub fn message(v: &Value, chat: &str, other: &str) -> Post {
  let id = v.str("mid").or_else(|| v.str("msgId")).unwrap_or_default();
  let from = v.str("from.uid").unwrap_or_default();
  let kind = v.i64("body.type").or_else(|| v.i64("type")).unwrap_or(0);
  let mut post = Post {
    id: message_id(chat, &id),
    kind: "message".into(),
    created_at: time(v, "time"),
    ..Post::default()
  };
  post.text = match kind {
    TEXT if v.i64("body.templateId") == Some(PHONE_TEMPLATE) => Some("[contact exchange]".into()),
    TEXT => text(v, "body.text").map(|t| plain(&t)),
    VOICE => Some("[voice]".into()),
    IMAGE => {
      let image = v.first_str(&["body.image.originImage.url", "body.image.tinyImage.url"]);
      post.media = image.map(Media::image).into_iter().collect();
      Some("[image]".into())
    }
    _ => Some(format!("[card {kind}]")),
  };
  // Cards may carry phone numbers, WeChat ids or résumés: `raw` keeps
  // the message's frame only.
  let mut raw = sanitize(v);
  if let Some(map) = raw.as_object_mut() {
    map.retain(|key, _| !["body", "from", "to", "pushText"].contains(&key.as_str()));
  }
  post.raw = Some(raw);
  let direction = if !other.is_empty() && from == other {
    "received"
  } else {
    "sent"
  };
  post.extra.insert("direction".into(), json!(direction));
  post.extra.insert("type".into(), json!(kind));
  post
}
