//! 知识星球 payloads → the shared model: topics as posts (talks, Q&A with
//! their answer, homework and its solutions), comments with the replies
//! the API nests in them, groups as collections and users.

mod text;

pub use text::{api_time, shifted};

use atp_core::{Collection, Comment, Media, Metrics, Post, User, Value, ValueExt, json};

use crate::api::WEB;

pub fn topic_url(id: &str) -> String {
  format!("{WEB}/topic/{id}")
}

pub fn group_url(id: &str) -> String {
  format!("{WEB}/group/{id}")
}

pub fn user_url(id: &str) -> String {
  format!("{WEB}/footprint/{id}")
}

/// A member as the API gives one (`owner`, `repliee`, `questionee` …).
pub fn user(v: &Value) -> Option<User> {
  let name = v.first_str(&["name", "alias"])?;
  let id = v.first_str(&["user_id", "uid"]).unwrap_or_default();
  let mut user = User {
    url: (!id.is_empty()).then(|| user_url(&id)),
    id,
    name,
    handle: v.str("unique_id"),
    avatar: v.str("avatar_url"),
    bio: v.str("introduction").map(|b| text::plain(&b)),
    location: v.str("location"),
    raw: Some(v.clone()),
    ..User::default()
  };
  user.stats.followers = v.count("followers_count");
  user.stats.following = v.count("followees_count");
  user.stats.posts = v.count("topics_count");
  if let Some(notes) = v.count("notes_count") {
    user.stats.other.insert("notes".into(), notes);
  }
  Some(user)
}

/// A group (星球): its members, topics, owner, kind and your membership.
pub fn group(v: &Value) -> Option<Collection> {
  let id = v.str("group_id")?;
  let mut group = Collection {
    kind: "group".into(),
    name: v.str("name").unwrap_or_else(|| id.clone()),
    description: v.str("description").map(|d| text::plain(&d)),
    url: Some(group_url(&id)),
    items: v.first_count(&["statistics.topics.topics_count", "statistics.topics_count"]),
    followers: v.first_count(&[
      "statistics.members.count",
      "statistics.subscriptions_count",
      "statistics.members_count",
    ]),
    owner: user(v.at("owner")).map(|mut owner| {
      owner.raw = None;
      owner
    }),
    id,
    raw: Some(v.clone()),
    ..Collection::default()
  };
  let fields = [
    ("type", "type"),
    ("category", "category.title"),
    ("created_at", "create_time"),
    ("latest_topic_at", "latest_topic_create_time"),
    ("joined_at", "user_specific.join_time"),
    ("expires_at", "user_specific.validity.end_time"),
  ];
  for (key, path) in fields {
    if let Some(value) = v.str(path) {
      group.extra.insert(key.into(), json!(value));
    }
  }
  Some(group)
}

/// The part of a topic that holds its text, by its type.
fn body<'a>(v: &'a Value, kind: &str) -> &'a Value {
  match kind {
    "q&a" => v.at("question"),
    "task" => v.at("task"),
    "solution" => v.at("solution"),
    _ => v.at("talk"),
  }
}

/// The pictures of a part, largest first offered.
fn images(part: &Value) -> Vec<Media> {
  part
    .list("images")
    .iter()
    .filter_map(|image| {
      let size = ["original", "large", "thumbnail"]
        .iter()
        .map(|k| image.at(k))
        .find(|s| s.str("url").is_some())?;
      let mut media = Media::image(size.str("url")?);
      media.width = size.u64("width").map(|w| w as u32);
      media.height = size.u64("height").map(|h| h as u32);
      media.size = size.u64("size");
      Some(media)
    })
    .collect()
}

/// The files attached to a part, as `extra.files` lists them.
fn files(part: &Value) -> Vec<Value> {
  part
    .list("files")
    .iter()
    .filter_map(|f| {
      Some(json!({
        "id": f.str("file_id")?,
        "name": f.str("name").unwrap_or_default(),
        "size": f.u64("size"),
        "downloads": f.u64("download_count"),
        "created_at": f.str("create_time"),
      }))
    })
    .collect()
}

/// The title a topic has of its own: an article's, or one the author set.
/// The API also gives the start of the text as `title`, which is left out.
fn title(v: &Value, part: &Value, text: &str) -> Option<String> {
  if let Some(article) = part.str("article.title") {
    return Some(text::plain(&article));
  }
  let title = text::plain(&v.str("title")?);
  let stem = title.trim_end_matches("...").trim();
  let flat = |s: &str| s.split_whitespace().collect::<String>();
  (!stem.is_empty() && !flat(text).starts_with(&flat(stem))).then_some(title)
}

/// A topic: its text (a Q&A's question and answer), author, pictures,
/// counts, group, files, article and video.
pub fn topic(v: &Value) -> Option<Post> {
  let id = v.str("topic_id")?;
  let kind = v.str("type").unwrap_or_default();
  let part = body(v, &kind);
  let raw_text = part.str("text").unwrap_or_default();
  let question = text::plain(&raw_text);
  let mut text = question.clone();
  let mut media = images(part);
  let answer = v.at("answer");
  if let Some(answer_text) = answer.str("text") {
    let by = user(answer.at("owner")).map(|u| u.name);
    let by = by.map(|name| format!(" ({name})")).unwrap_or_default();
    text = format!("Q: {question}\n\nA{by}: {}", text::plain(&answer_text));
    media.extend(images(answer));
  }
  let mut tags = text::hashtags(&raw_text);
  for tag in text::hashtags(&answer.str("text").unwrap_or_default()) {
    if !tags.contains(&tag) {
      tags.push(tag);
    }
  }
  let mut post = Post {
    title: title(v, part, &question),
    kind: kind_name(&kind).into(),
    url: Some(topic_url(&id)),
    author: user(part.at("owner")).map(|mut u| {
      u.raw = None;
      u
    }),
    created_at: v.str("create_time").as_deref().and_then(text::time),
    metrics: metrics(v),
    media,
    tags,
    text: (!text.is_empty()).then_some(text),
    id,
    raw: Some(v.clone()),
    ..Post::default()
  };
  post.extra = extra(v, part);
  Some(post)
}

/// The shared names of the topic types.
fn kind_name(kind: &str) -> &str {
  match kind {
    "talk" | "" => "topic",
    "q&a" => "question",
    other => other,
  }
}

fn metrics(v: &Value) -> Metrics {
  let mut metrics = Metrics {
    views: v.first_count(&["reading_count", "readers_count"]),
    likes: v.count("likes_count"),
    comments: v.count("comments_count"),
    ..Metrics::default()
  };
  for (key, path) in [("rewards", "rewards_count"), ("readers", "readers_count")] {
    if let Some(n) = v.count(path) {
      metrics.other.insert(key.into(), n);
    }
  }
  metrics
}

/// What a topic has beyond the shared fields.
fn extra(v: &Value, part: &Value) -> atp_core::Extra {
  let mut extra = atp_core::Extra::new();
  let group = v.at("group");
  if let Some(gid) = group.str("group_id").or_else(|| v.str("group_id")) {
    let mut g = json!({"id": gid, "url": group_url(&gid)});
    if let Some(name) = group.str("name") {
      g["name"] = json!(name);
    }
    extra.insert("group".into(), g);
  }
  for (key, path) in [
    ("digested", "digested"),
    ("sticky", "sticky"),
    ("liked", "user_specific.liked"),
  ] {
    if v.bool(path) == Some(true) {
      extra.insert(key.into(), json!(true));
    }
  }
  let files = files(part);
  if !files.is_empty() {
    extra.insert("files".into(), json!(files));
  }
  if let Some(url) = part.first_str(&["article.article_url", "article.inline_article_url"]) {
    extra.insert(
      "article".into(),
      json!({"title": part.str("article.title"), "url": url}),
    );
  }
  if let Some(video) = part.str("video.video_id") {
    extra.insert(
      "video".into(),
      json!({"id": video, "duration": part.f64("video.duration"), "cover": part.str("video.cover.url")}),
    );
  }
  extra.extend(question_extra(v, part));
  extra
}

/// A Q&A's other side: who was asked, whether it is answered, the answer.
fn question_extra(v: &Value, part: &Value) -> atp_core::Extra {
  let mut extra = atp_core::Extra::new();
  if v.str("type").as_deref() != Some("q&a") {
    return extra;
  }
  if let Some(asked) = user(part.at("questionee")) {
    extra.insert(
      "questionee".into(),
      json!({"id": asked.id, "name": asked.name}),
    );
  }
  if part.bool("anonymous") == Some(true) {
    extra.insert("anonymous".into(), json!(true));
  }
  let answered = v.bool("answered").unwrap_or(v.at("answer").is_object());
  extra.insert("answered".into(), json!(answered));
  let answer = v.at("answer");
  if let Some(text) = answer.str("text") {
    let by = user(answer.at("owner"));
    extra.insert(
      "answer".into(),
      json!({
        "author": by.map(|u| json!({"id": u.id, "name": u.name})),
        "text": text::plain(&text),
      }),
    );
  }
  extra
}

/// A comment with the replies nested in it.
pub fn comment(v: &Value) -> Option<Comment> {
  let id = v.str("comment_id")?;
  let raw_text = v.str("text").unwrap_or_default();
  let mut comment = Comment {
    id,
    author: user(v.at("owner")).map(|mut u| {
      u.raw = None;
      u
    }),
    text: text::plain(&raw_text),
    created_at: v.str("create_time").as_deref().and_then(text::time),
    likes: v.count("likes_count"),
    reply_count: v.count("replies_count"),
    reply_to: v.str("repliee.name"),
    replies: v
      .list("replied_comments")
      .iter()
      .filter_map(comment)
      .collect(),
    raw: Some(v.clone()),
    ..Comment::default()
  };
  let pictures: Vec<String> = images(v).into_iter().map(|m| m.url).collect();
  if !pictures.is_empty() {
    comment.extra.insert("images".into(), json!(pictures));
  }
  if v.bool("sticky") == Some(true) || v.bool("sticked") == Some(true) {
    comment.extra.insert("sticky".into(), json!(true));
  }
  if let Some(n) = v.count("rewards_count").filter(|n| *n > 0) {
    comment.extra.insert("rewards".into(), json!(n));
  }
  Some(comment)
}
