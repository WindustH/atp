//! 1Point3Acres payloads → the shared model: threads as posts (hiring posts
//! of the 内推 boards as jobs, [`jobs`]), replies as comments with the
//! replies nested under them, users, and boards as collections. Texts come
//! as Discuz BBCode ([`bbcode`]).

pub mod bbcode;
mod jobs;

use atp_core::text::{from_secs, html_to_text};
use atp_core::{Collection, Comment, Media, Metrics, Post, User, UserStats, Value, ValueExt, json};

use crate::api::WWW;

/// Where the site keeps avatars.
const AVATARS: &str = "https://avatar.1p3a.com";

pub fn thread_url(tid: &str) -> String {
  format!("{WWW}/bbs/thread-{tid}-1-1.html")
}

pub fn user_url(uid: u64) -> String {
  format!("{WWW}/home/u/{uid}")
}

pub fn board_url(fid: u64) -> String {
  format!("{WWW}/bbs/forum-{fid}-1.html")
}

/// The avatar of a user, where the site's pages take it from:
/// `001/35/85/42_avatar_middle.jpg` for 1358542.
fn avatar(uid: u64) -> String {
  let digits = format!("{uid:09}");
  format!(
    "{AVATARS}/{}/{}/{}/{}_avatar_middle.jpg",
    &digits[..3],
    &digits[3..5],
    &digits[5..7],
    &digits[7..]
  )
}

/// Subjects and names come HTML-escaped (`&#10024;企业直聘`).
pub fn plain(text: &str) -> String {
  html_to_text(text.trim())
}

fn time(v: &Value, path: &str) -> Option<jiff::Timestamp> {
  v.i64(path).and_then(from_secs)
}

/// A user known by number and name (`me`, profiles).
fn known_user(uid: u64, name: String) -> User {
  User {
    id: uid.to_string(),
    name,
    url: Some(user_url(uid)),
    avatar: Some(avatar(uid)),
    ..User::default()
  }
}

/// The author of a thread or reply; anonymous posts (`地里匿名用户…`) have
/// no number or link.
pub fn author(v: &Value) -> Option<User> {
  let name = v.str("author").map(|n| plain(&n))?;
  let uid = v.u64("authorid").filter(|uid| *uid > 0);
  let mut user = match uid {
    Some(uid) => known_user(uid, name),
    None => User {
      name,
      ..User::default()
    },
  };
  if uid.is_none() || v.bool("anonymous") == Some(true) {
    user.extra.insert("anonymous".into(), json!(true));
  }
  Some(user)
}

/// The account behind the session (tRPC `user.me`).
pub fn me(v: &Value) -> Option<User> {
  let uid = v.u64("uid").filter(|uid| *uid > 0)?;
  let mut user = known_user(uid, v.str("username").unwrap_or_else(|| uid.to_string()));
  if let Some(group) = v.u64("groupid") {
    user.extra.insert("group".into(), json!(group));
  }
  user.raw = Some(v.clone());
  Some(user)
}

/// A public profile (`/api/users/<uid>/profile`, its `data`).
pub fn profile(v: &Value) -> Option<User> {
  let mut user = me(v)?;
  user.created_at = time(v, "regdate");
  user.stats = UserStats {
    followers: v.u64("user_count.follower"),
    following: v.u64("user_count.following"),
    posts: v.u64("user_count.threads"),
    ..UserStats::default()
  };
  if let Some(n) = v.u64("user_count.posts") {
    user.stats.other.insert("all_posts".into(), n);
  }
  if let Some(n) = v.i64("credits") {
    user.extra.insert("credits".into(), json!(n));
  }
  Some(user)
}

/// The board of a thread: its number, name and (when read whole) section.
fn board(v: &Value) -> Option<Value> {
  let fid = v.u64("fid")?;
  let names = v.at("forum_name");
  let name = names
    .as_str()
    .map(str::to_owned)
    .or_else(|| names.str("name"))
    .map(|n| plain(&n));
  let mut board = json!({"id": fid, "name": name});
  if let Some(section) = names.str("section.name") {
    board["section"] = json!(plain(&section));
  }
  Some(board)
}

/// Topic tags (companies, places, schools) by name.
fn tag_names(v: &Value) -> Vec<String> {
  let mut names: Vec<String> = Vec::new();
  for tag in v.list("topic_tag").iter().chain(v.list("tags")) {
    let name = tag
      .str("tagname")
      .or_else(|| tag.as_str().map(str::to_owned));
    if let Some(name) = name.map(|n| plain(&n)).filter(|n| !n.is_empty())
      && !names.contains(&name)
    {
      names.push(name);
    }
  }
  names
}

fn images(v: &Value) -> Vec<Media> {
  v.list("images")
    .iter()
    .filter_map(|image| {
      let mut media = Media::image(image.str("url")?);
      media.width = image.u64("width").map(|w| w as u32);
      media.height = image.u64("height").map(|h| h as u32);
      Some(media)
    })
    .collect()
}

/// A thread of a list (with its `summary` where asked for) or read whole
/// (with its first post's text).
pub fn thread(v: &Value) -> Post {
  let tid = v.str("tid").unwrap_or_default();
  let body = v.str("message_bbcode").map(|m| bbcode::parse(&m));
  let summary = v.str("summary").map(|s| plain(&s));
  let mut post = Post {
    kind: "thread".into(),
    title: v.str("subject").map(|s| plain(&s)),
    text: body
      .as_ref()
      .map(|b| b.text.clone())
      .or(summary)
      .filter(|t| !t.is_empty()),
    url: Some(thread_url(&tid)),
    author: author(v),
    created_at: time(v, "dateline"),
    metrics: Metrics {
      views: v.u64("views"),
      likes: v.u64("votes.upvote").or_else(|| v.u64("recommend_add")),
      comments: v.u64("replies"),
      favorites: v.u64("favtimes"),
      ..Metrics::default()
    },
    media: images(v),
    tags: tag_names(v),
    raw: Some(v.clone()),
    id: tid,
    ..Post::default()
  };
  for (key, path) in [
    ("downvotes", "votes.downvote"),
    ("heat", "heats"),
    ("rate", "rate"),
  ] {
    if let Some(n) = v.u64(path).filter(|n| *n > 0) {
      post.metrics.other.insert(key.into(), n);
    }
  }
  if let Some(body) = &body {
    post.media.extend(body.images.iter().map(Media::image));
  }
  thread_extra(&mut post, v, body.as_ref());
  if jobs::is_job(v) {
    jobs::fill(&mut post, v);
  }
  post
}

/// What a thread is besides its text: board, type, marks, last reply,
/// hidden parts.
fn thread_extra(post: &mut Post, v: &Value, body: Option<&bbcode::Body>) {
  let extra = &mut post.extra;
  if let Some(board) = board(v) {
    extra.insert("board".into(), board);
  }
  if let Some(kind) = v
    .str("thread_type")
    .map(|t| plain(&t))
    .filter(|t| !t.is_empty())
  {
    extra.insert("type".into(), json!(kind));
  }
  if v.u64("digest").is_some_and(|d| d > 0) {
    extra.insert("digest".into(), json!(true));
  }
  if v.i64("displayorder").is_some_and(|d| d > 0) {
    extra.insert("pinned".into(), json!(true));
  }
  if v.u64("close_status").is_some_and(|c| c > 0) {
    extra.insert("closed".into(), json!(true));
  }
  if let Some(at) = time(v, "lastpost") {
    extra.insert("last_reply_at".into(), json!(at));
  }
  if let Some(name) = v.str("lastposter") {
    extra.insert("last_poster".into(), json!(plain(&name)));
  }
  if !v.at("threadpay").is_null() {
    extra.insert("paid".into(), json!(true));
  }
  if let Some(body) = body.filter(|b| b.hidden) {
    extra.insert("hidden".into(), json!(true));
    if let Some(credits) = body.credits {
      extra.insert("hidden_credits".into(), json!(credits));
    }
  }
}

/// A reply and, nested, the replies the site sends with it.
pub fn comment(v: &Value) -> Comment {
  let body = bbcode::parse(&v.str("message_bbcode").unwrap_or_default());
  let mut text = body.text.clone();
  for image in &body.images {
    if !text.is_empty() {
      text.push('\n');
    }
    text.push_str(&format!("[image] {image}"));
  }
  let mut comment = Comment {
    id: v.str("pid").unwrap_or_default(),
    author: author(v),
    text,
    created_at: time(v, "dateline"),
    likes: v.u64("votes.upvote"),
    reply_count: v.u64("replies.count"),
    reply_to: v
      .str("quote_author.author")
      .map(|n| plain(&n))
      .filter(|n| !n.is_empty()),
    replies: v.list("replies.data").iter().map(comment).collect(),
    raw: Some(v.clone()),
    ..Comment::default()
  };
  if let Some(n) = v.u64("votes.downvote").filter(|n| *n > 0) {
    comment.extra.insert("downvotes".into(), json!(n));
  }
  if v.bool("votes.need_fold") == Some(true) {
    comment.extra.insert("folded".into(), json!(true));
  }
  if body.hidden {
    comment.extra.insert("hidden".into(), json!(true));
  }
  comment
}

/// A section (`group`) or board (`forum`, `sub`) of `/api/forums`;
/// `path` names where it sits (`海外求职 › 海外面经`).
pub fn board_entry(v: &Value, path: Option<String>) -> Collection {
  let fid = v.u64("fid").unwrap_or_default();
  let section = v.str("type").as_deref() == Some("group");
  let mut board = Collection {
    id: fid.to_string(),
    kind: if section { "section" } else { "board" }.into(),
    name: v.str("name").map(|n| plain(&n)).unwrap_or_default(),
    description: path,
    url: Some(board_url(fid)),
    raw: Some(v.clone()),
    ..Collection::default()
  };
  if let Some(parent) = v.u64("fup").filter(|p| *p > 0) {
    board.extra.insert("parent".into(), json!(parent));
  }
  let types = v.at("forum_field.thread_types");
  if types.as_object().is_some_and(|t| !t.is_empty()) {
    board.extra.insert("types".into(), types.clone());
  }
  board
}
