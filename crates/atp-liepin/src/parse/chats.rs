//! Recruiter chats as the site's IM lists them: a contact
//! (`oppositeImId`, name, title, company, unread count) is a chat, a
//! message (`msgId`, `msgTime`, `direction` and a `payload` holding
//! `{ext, bodies: [{type, msg}]}`) is a post. Cards trading phone numbers
//! or WeChat ids are shown as such, without their contents.

use atp_core::chat::message_id;
use atp_core::{Collection, Media, Post, Value, ValueExt, json};

use super::{image, sanitize, time};

/// Message kinds (`ext.extType`) that trade contact details.
const EXCHANGE_KINDS: &[i64] = &[206, 207];

/// A contact of the chat list.
pub fn chat(v: &Value) -> Collection {
  let id = v.str("oppositeImId").unwrap_or_default();
  let title = v.str("title");
  let company = v.str("compName");
  let mut chat = Collection {
    kind: "dm".into(),
    name: v.str("name").unwrap_or_else(|| id.clone()),
    description: match (&title, &company) {
      (Some(t), Some(c)) => Some(format!("{t} · {c}")),
      (t, c) => t.clone().or_else(|| c.clone()),
    },
    raw: Some(sanitize(v)),
    id,
    ..Collection::default()
  };
  let extra = &mut chat.extra;
  if let Some(n) = v.u64("unReadCnt").filter(|n| *n > 0) {
    extra.insert("unread".into(), json!(n));
  }
  if let Some(at) = time(v.at("latestMsgTime")).or_else(|| time(v.at("msgTime"))) {
    extra.insert("last_message_at".into(), json!(at));
  }
  if let Some(photo) = v.str("photo").as_deref().and_then(image) {
    extra.insert("avatar".into(), json!(photo));
  }
  for (key, value) in [("company", company), ("title", title)] {
    if let Some(value) = value {
      extra.insert(key.into(), json!(value));
    }
  }
  chat
}

/// The `{ext, bodies}` a message's `payload` string holds.
fn payload(v: &Value) -> Value {
  match v.at("payload") {
    Value::String(s) => serde_json::from_str(s).unwrap_or_default(),
    other => other.clone(),
  }
}

/// A message of a chat, its text read from the payload.
pub fn message(v: &Value, chat: &str) -> Post {
  let body = payload(v);
  let first = body.at("bodies.0");
  let kind = first.str("type").unwrap_or_else(|| "txt".into());
  let exchange = body
    .i64("ext.extType")
    .is_some_and(|k| EXCHANGE_KINDS.contains(&k));
  let mut post = Post {
    id: message_id(chat, &v.str("msgId").unwrap_or_default()),
    kind: "message".into(),
    created_at: time(v.at("msgTime")),
    ..Post::default()
  };
  post.text = if exchange {
    Some("[contact exchange]".into())
  } else if kind == "img" {
    post.media = first.str("url").map(Media::image).into_iter().collect();
    Some("[image]".into())
  } else {
    first.str("msg")
  };
  // The payload stays out of `raw`: cards may carry contact details.
  let mut raw = sanitize(v);
  if let Some(map) = raw.as_object_mut() {
    map.remove("payload");
  }
  post.raw = Some(raw);
  if let Some(direction) = v.str("direction") {
    post.extra.insert("direction".into(), json!(direction));
  }
  post.extra.insert("type".into(), json!(kind));
  post
}
