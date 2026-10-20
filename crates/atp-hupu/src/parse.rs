//! Hupu payloads → the shared model: threads as posts (from lists and
//! from thread pages), replies as comments, boards as collections and
//! mentions as notifications. Texts come as HTML.

use std::sync::LazyLock;

use atp_core::text::{from_beijing, from_unix, html_to_text};
use atp_core::{
  Collection, Comment, Media, Metrics, Notification, Post, User, Value, ValueExt, json,
};
use jiff::Timestamp;
use regex::Regex;

use crate::api::BBS;

static IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<img\b[^>]*>").unwrap());
static ATTR: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"(?is)\b(data-origin|data-src|src)\s*=\s*["']([^"']+)["']"#).unwrap()
});

pub fn thread_url(tid: &str) -> String {
  format!("{BBS}/{tid}.html")
}

/// A link of the site from a path it gives (`/history`) or a whole link.
pub fn site_url(path: &str) -> String {
  if path.starts_with("http") {
    path.to_owned()
  } else if let Some(rest) = path.strip_prefix("//") {
    format!("https://{rest}")
  } else {
    format!("{BBS}/{}", path.trim_start_matches('/'))
  }
}

fn time(v: &Value, path: &str) -> Option<Timestamp> {
  v.i64(path).and_then(from_unix)
}

/// A time the site writes out: milliseconds or seconds, or Beijing time
/// as text (`2026-10-20 12:00`).
pub fn time_text(v: &Value, path: &str) -> Option<Timestamp> {
  time(v, path).or_else(|| v.str(path).as_deref().and_then(from_beijing))
}

/// A picture link without the resizing the page asks of the image host.
fn original(src: &str) -> String {
  let src = src.trim();
  let src = if let Some(rest) = src.strip_prefix("//") {
    format!("https://{rest}")
  } else {
    src.to_owned()
  };
  match src.split_once("?x-oss-process") {
    Some((base, _)) => base.to_owned(),
    None => src,
  }
}

/// The pictures of a text, in order.
fn images(html: &str) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for tag in IMG.find_iter(html) {
    let mut found: Option<(usize, String)> = None;
    for caps in ATTR.captures_iter(tag.as_str()) {
      let rank = match &caps[1].to_ascii_lowercase()[..] {
        "data-origin" => 0,
        "data-src" => 1,
        _ => 2,
      };
      if found.as_ref().is_none_or(|(r, _)| rank < *r) {
        found = Some((rank, caps[2].to_owned()));
      }
    }
    if let Some((_, src)) = found
      && !src.starts_with("data:")
    {
      let src = original(&src);
      if !out.contains(&src) {
        out.push(src);
      }
    }
  }
  out
}

/// An author or user as the pages give them: `puid` (the number), `puname`
/// and the profile link (by `euid`).
pub fn user(v: &Value) -> Option<User> {
  let name = v.first_str(&["puname", "username", "userName"])?;
  let mut user = User {
    id: v.str("puid").unwrap_or_default(),
    name,
    url: v.str("url").map(|u| site_url(&u)),
    avatar: v.str("header").map(|u| original(&u)),
    raw: Some(v.clone()),
    ..User::default()
  };
  if let Some(level) = v.u64("level") {
    user.extra.insert("level".into(), json!(level));
  }
  if let Some(euid) = v.str("euid") {
    user.extra.insert("euid".into(), json!(euid));
  }
  Some(user)
}

/// The board a thread is in: its place on the site, name, number and zone.
fn board_of(v: &Value) -> Option<Value> {
  let topic = v.at("topic");
  let name = topic.str("name")?;
  let mut board = json!({"name": name});
  if let Some(url) = topic.str("url") {
    board["id"] = json!(url.trim_matches('/'));
    board["url"] = json!(site_url(&url));
  }
  if let Some(id) = topic.str("topicId").or_else(|| v.str("topicId")) {
    board["topic_id"] = json!(id);
  }
  if let Some(fid) = topic.str("fid").or_else(|| v.str("fid")) {
    board["fid"] = json!(fid);
  }
  if let Some(zone) = topic.str("cateName") {
    board["zone"] = json!(zone);
  }
  Some(board)
}

/// A thread of a list (title and counts) or of its page (with its text).
pub fn thread(v: &Value) -> Post {
  let tid = v.str("tid").unwrap_or_default();
  let content = v.str("content");
  let text = content
    .as_deref()
    .map(html_to_text)
    .or_else(|| v.str("desc").map(|d| html_to_text(&d)))
    .filter(|t| !t.is_empty());
  let mut media: Vec<Media> = content
    .as_deref()
    .map(images)
    .unwrap_or_default()
    .into_iter()
    .map(Media::image)
    .collect();
  if let Some(video) = v.str("video") {
    media.insert(0, Media::video(original(&video)));
  } else if let Some(cover) = v.str("cover").or_else(|| v.str("videoCover"))
    && media.is_empty()
  {
    media.push(Media::image(original(&cover)));
  }
  let mut post = Post {
    kind: "thread".into(),
    title: v.str("title").map(|t| html_to_text(&t)),
    text,
    url: Some(thread_url(&tid)),
    author: user(v.at("author")),
    created_at: time(v, "createdAt"),
    metrics: Metrics {
      views: v.u64("read"),
      likes: v.u64("recommend"),
      comments: v.u64("replies"),
      ..Metrics::default()
    },
    media,
    raw: Some(v.clone()),
    id: tid,
    ..Post::default()
  };
  if let Some(lights) = v.u64("lights") {
    post.metrics.other.insert("lights".into(), lights);
  }
  thread_extra(&mut post, v);
  post
}

fn thread_extra(post: &mut Post, v: &Value) {
  let extra = &mut post.extra;
  if let Some(board) = board_of(v) {
    extra.insert("board".into(), board);
  }
  if let Some(at) = time(v, "repliedAt") {
    extra.insert("last_reply_at".into(), json!(at));
  }
  if let Some(place) = v.str("location") {
    extra.insert("location".into(), json!(place));
  }
  if v.bool("hasVideo") == Some(true) {
    extra.insert("video".into(), json!(true));
  }
  if v.i64("isLock").is_some_and(|l| l > 0) {
    extra.insert("locked".into(), json!(true));
  }
  if let Some(client) = v.str("client") {
    extra.insert("client".into(), json!(client));
  }
}

/// A reply (回帖), with the reply it quotes in `extra.quote`.
pub fn reply(v: &Value) -> Comment {
  let content = v.str("content").unwrap_or_default();
  let mut text = html_to_text(&content);
  for image in images(&content) {
    if !text.is_empty() {
      text.push('\n');
    }
    text.push_str(&format!("[image] {image}"));
  }
  let quote = v.at("quote");
  let mut comment = Comment {
    id: v.str("pid").unwrap_or_default(),
    author: user(v.at("author")),
    text,
    created_at: time(v, "createdAt"),
    likes: v.u64("allLightCount").or_else(|| v.u64("count")),
    reply_count: v.u64("replyNum"),
    reply_to: quote.str("author.puname"),
    location: v.str("location"),
    raw: Some(v.clone()),
    ..Comment::default()
  };
  if let Some(pid) = quote.str("pid") {
    let quoted = html_to_text(&quote.str("content").unwrap_or_default());
    comment.extra.insert(
      "quote".into(),
      json!({"id": pid, "author": quote.str("author.puname"), "text": quoted}),
    );
  }
  if v.bool("isStarter") == Some(true) {
    comment.extra.insert("op".into(), json!(true));
  }
  for (key, flag) in [("hidden", "isHidden"), ("deleted", "isDelete")] {
    if v.bool(flag) == Some(true) {
      comment.extra.insert(key.into(), json!(true));
    }
  }
  comment
}

/// A board (版块) as the zones list it, with the zone it is in.
pub fn board(v: &Value, zone: Option<&str>) -> Collection {
  let url = v.str("url").unwrap_or_default();
  let mut board = Collection {
    id: url.trim_matches('/').to_owned(),
    kind: "board".into(),
    name: v.str("name").unwrap_or_default(),
    description: v.str("desc").filter(|d| !d.trim().is_empty()),
    url: Some(site_url(&url)),
    raw: Some(v.clone()),
    ..Collection::default()
  };
  if let Some(id) = v.str("topicId") {
    board.extra.insert("topic_id".into(), json!(id));
  }
  if let Some(zone) = zone.map(str::to_owned).or_else(|| v.str("cateName")) {
    board.extra.insert("zone".into(), json!(zone));
  }
  if let Some(count) = v.u64("count") {
    board.extra.insert("count".into(), json!(count));
  }
  board
}

/// A reply that mentions the account (提到我的): its text, with the
/// reply it answers quoted under it (`> …`), and a link to it in its
/// thread.
pub fn mention(v: &Value) -> Notification {
  let tid = v.str("tid").unwrap_or_default();
  let pid = v.str("pid").unwrap_or_default();
  let mut url = (!tid.is_empty()).then(|| thread_url(&tid));
  if let Some(link) = url.as_mut().filter(|_| !pid.is_empty()) {
    link.push_str(&format!("?pid={pid}"));
  }
  let mut text = html_to_text(&v.str("postContent").unwrap_or_default());
  if let Some(quote) = v
    .str("quoteContent")
    .map(|q| html_to_text(&q))
    .filter(|q| !q.is_empty())
  {
    text.push_str(&format!("\n> {}", quote.replace('\n', "\n> ")));
  }
  Notification {
    id: if pid.is_empty() { tid } else { pid },
    kind: "mention".into(),
    text,
    actor: v.str("username").map(|name| User {
      id: v.str("puid").unwrap_or_default(),
      name,
      ..User::default()
    }),
    target: v.str("threadTitle"),
    url,
    created_at: time_text(v, "publishTime"),
    raw: Some(v.clone()),
    ..Notification::default()
  }
}
