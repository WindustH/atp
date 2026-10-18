//! Eleduck payloads → the shared model: posts (job posts and discussions),
//! users, comments with their nested replies, messages as notifications,
//! categories and tags as collections. What a job post says about the job
//! goes into the shared job keys of `Post.extra` ([`jobs`]).
//!
//! Contact details some payloads carry (`contact_info`, phone numbers,
//! e-mail and WeChat fields) are dropped from `raw` ([`sanitize`]): ATP
//! never passes them on.

mod jobs;

use atp_core::text::{html_to_text, parse_time};
use atp_core::{
  Collection, Comment, Metrics, Notification, Post, User, UserStats, Value, ValueExt, json,
};

use crate::api::WEB;

/// Keys of contact details, dropped wherever they appear.
const CONTACT_KEYS: &[&str] = &[
  "contact_info",
  "contact_methods",
  "email_address",
  "phone_number",
  "phone",
  "email",
  "wechat",
  "wechat_id",
];

/// The payload without contact details.
pub fn sanitize(v: &Value) -> Value {
  match v {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .filter(|(key, _)| !CONTACT_KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), sanitize(value)))
        .collect(),
    ),
    Value::Array(items) => Value::Array(items.iter().map(sanitize).collect()),
    other => other.clone(),
  }
}

/// The next page number of a `{…, pager}` listing.
pub fn next_page(v: &Value) -> Option<String> {
  v.u64("pager.next_page").map(|page| page.to_string())
}

pub fn post_url(id: &str) -> String {
  format!("{WEB}/posts/{id}")
}

fn time(v: &Value, path: &str) -> Option<jiff::Timestamp> {
  v.str(path).and_then(|t| parse_time(&t))
}

/// The source text (Markdown as written), else the rendered HTML as text.
/// Some older source texts are HTML themselves (`<p>…</p>`).
fn content(v: &Value) -> Option<String> {
  v.str("raw_content")
    .map(|text| text.replace("\r\n", "\n").trim().to_owned())
    .map(|text| {
      if text.starts_with('<') {
        html_to_text(&text)
      } else {
        text
      }
    })
    .filter(|text| !text.is_empty())
    .or_else(|| v.str("content").map(|html| html_to_text(&html)))
    .filter(|text| !text.is_empty())
}

/// A user as lists show them: name, avatar, level.
pub fn user(v: &Value) -> User {
  let id = v.str("id").unwrap_or_default();
  let mut user = User {
    url: Some(format!("{WEB}/users/{id}")),
    name: v
      .str("nickname")
      .map(|name| name.trim().to_owned())
      .unwrap_or_else(|| id.clone()),
    avatar: v.str("avatar_url"),
    bio: v
      .first_str(&["bio", "tagline"])
      .filter(|bio| !bio.trim().is_empty()),
    location: v.str("location").filter(|l| !l.is_empty()),
    created_at: time(v, "created_at"),
    id,
    ..User::default()
  };
  let level = v.u64("level").or_else(|| v.u64("level_info.level"));
  if let Some(level) = level {
    user.extra.insert("level".into(), json!(level));
  }
  for key in ["tagline", "tag_identity", "job_title"] {
    if let Some(value) = v.str(key).filter(|s| !s.trim().is_empty()) {
      user.extra.insert(key.into(), json!(value));
    }
  }
  user
}

/// A profile (`/users/<id>`, `/profile/info`): counts and tags too.
pub fn profile(v: &Value) -> User {
  let mut user = user(v);
  user.stats = UserStats {
    posts: v.u64("posts_count"),
    likes: v.u64("posts_upvoted_count"),
    ..UserStats::default()
  };
  for (key, path) in [
    ("comments", "comments_count"),
    ("comment_likes", "comments_upvoted_count"),
  ] {
    if let Some(n) = v.u64(path) {
      user.stats.other.insert(key.into(), n);
    }
  }
  // 电量 (the site's credits) and experience points.
  for key in ["ele", "exp"] {
    if let Some(n) = v.u64(key) {
      user.extra.insert(key.into(), json!(n));
    }
  }
  if let Some(name) = v.str("level_info.name") {
    user.extra.insert("level_name".into(), json!(name));
  }
  let tags = tag_names(v);
  if !tags.is_empty() {
    user.extra.insert("tags".into(), json!(tags));
  }
  user.raw = Some(sanitize(v));
  user
}

/// The names of the tags, each once (several groups have an 其他).
fn tag_names(v: &Value) -> Vec<String> {
  let mut names: Vec<String> = Vec::new();
  for name in v.list("tags").iter().filter_map(|t| t.str("name")) {
    if !names.contains(&name) {
      names.push(name);
    }
  }
  names
}

/// A post of a listing (with its `summary`) or read whole (with its text).
pub fn post(v: &Value) -> Post {
  let id = v.str("id").unwrap_or_default();
  let category = v.at("category");
  let job = category.str("kind").as_deref() == Some("work");
  let author = v.at("user");
  let mut post = Post {
    url: Some(post_url(&id)),
    kind: if job { "job" } else { "post" }.into(),
    title: v.first_str(&["title", "full_title"]),
    text: content(v).or_else(|| v.str("summary").filter(|s| !s.is_empty())),
    author: (!author.is_null()).then(|| user(author)),
    created_at: time(v, "published_at"),
    updated_at: time(v, "modified_at"),
    metrics: Metrics {
      views: v.u64("views_count"),
      likes: v.u64("upvotes_count"),
      comments: v.u64("comments_count"),
      favorites: v.u64("marks_count"),
      ..Metrics::default()
    },
    tags: tag_names(v),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  if let Some(n) = v.u64("downvotes_count").filter(|n| *n > 0) {
    post.metrics.other.insert("downvotes".into(), n);
  }
  if !category.is_null() {
    post.extra.insert(
      "category".into(),
      json!({"id": category.u64("id"), "name": category.str("name"), "code": category.str("code")}),
    );
  }
  for key in ["closed", "pinned", "featured"] {
    if v.bool(key) == Some(true) {
      post.extra.insert(key.into(), json!(true));
    }
  }
  if job {
    jobs::fill(&mut post, v);
  }
  post
}

/// A comment and, nested, the replies Eleduck sends with it.
pub fn comment(v: &Value) -> Comment {
  let id = v.str("id").unwrap_or_default();
  let author = v.at("user");
  let mut comment = Comment {
    author: (!author.is_null()).then(|| user(author)),
    text: content(v).unwrap_or_default(),
    created_at: time(v, "published_at"),
    likes: v.u64("upvotes_count"),
    reply_count: v.u64("replies_count"),
    reply_to: v.str("parent_user.nickname"),
    replies: v.list("replies").iter().map(comment).collect(),
    raw: Some(sanitize(v)),
    ..Comment::default()
  };
  if let Some(post) = v.str("post_id") {
    comment.extra.insert(
      "url".into(),
      json!(format!("{}#comment-{id}", post_url(&post))),
    );
  }
  if let Some(n) = v.u64("downvotes_count").filter(|n| *n > 0) {
    comment.extra.insert("downvotes".into(), json!(n));
  }
  comment.id = id;
  comment
}

/// A message (`/messages`): its kind read from Eleduck's, the post or job
/// it is about, and the comment's text for comments and replies.
pub fn notification(v: &Value) -> Notification {
  let kind = v.str("kind").unwrap_or_default();
  let details = v.at("details");
  let actor = v.at("user");
  let post = details.at("post");
  let opening = details.at("opening");
  let text = details
    .str("comment.content")
    .map(|html| html_to_text(&html))
    .filter(|text| !text.is_empty())
    .unwrap_or_else(|| kind.replace('_', " "));
  let url = post.str("id").map(|id| post_url(&id)).or_else(|| {
    opening
      .str("eleduck_post_id")
      .map(|id| format!("{WEB}/tposts/{id}"))
  });
  Notification {
    id: v.str("id").unwrap_or_default(),
    kind: notification_kind(&kind).into(),
    text,
    actor: (!actor.is_null()).then(|| user(actor)),
    target: post.str("title").or_else(|| opening.str("title")),
    url,
    created_at: time(v, "created_at"),
    raw: Some(sanitize(v)),
    ..Notification::default()
  }
}

/// The shared kind of one of Eleduck's message kinds.
fn notification_kind(kind: &str) -> &'static str {
  match kind {
    "post_comment_created" | "post_comment_created_with_ele" => "comment",
    "post_comment_replied" => "reply",
    "post_comment_voted" | "post_comment_voted_with_ele" => "like",
    "post_marked" => "favorite",
    "user_viewed" => "view",
    k if k.starts_with("job_application_") => "application",
    k if k.starts_with("talent_") => "application",
    _ => "system",
  }
}

/// A category (版块).
pub fn category(v: &Value) -> Collection {
  let id = v.u64("id").map(|id| id.to_string()).unwrap_or_default();
  let mut category = Collection {
    url: Some(format!("{WEB}/categories/{id}")),
    kind: "category".into(),
    name: v.str("name").unwrap_or_default(),
    description: v.str("description").filter(|d| !d.trim().is_empty()),
    items: v.u64("posts_count"),
    followers: v.u64("followed_count"),
    raw: Some(sanitize(v)),
    id,
    ..Collection::default()
  };
  for (key, path) in [("code", "code"), ("type", "kind")] {
    if let Some(value) = v.str(path) {
      category.extra.insert(key.into(), json!(value));
    }
  }
  if v.bool("paid") == Some(true) {
    category.extra.insert("paid".into(), json!(true));
  }
  if let Some(cost) = v.u64("post_ele_cost") {
    category.extra.insert("post_cost".into(), json!(cost));
  }
  let groups: Vec<String> = v
    .list("tag_groups")
    .iter()
    .filter_map(|g| g.str("code"))
    .collect();
  if !groups.is_empty() {
    category.extra.insert("tag_groups".into(), json!(groups));
  }
  category
}

/// The tags of a category's tag groups, each with its group.
pub fn tags(category: &Value) -> Vec<Collection> {
  let mut out = Vec::new();
  for group in category.list("tag_groups") {
    for tag in group.list("tags") {
      let mut item = Collection {
        id: tag.u64("id").map(|id| id.to_string()).unwrap_or_default(),
        kind: "tag".into(),
        name: tag.str("name").unwrap_or_default(),
        ..Collection::default()
      };
      item.extra.insert("group".into(), json!(group.str("code")));
      item
        .extra
        .insert("group_name".into(), json!(group.str("name")));
      out.push(item);
    }
  }
  out
}

/// One of your job applications: the job post, with the application's
/// state in `extra`.
pub fn application(v: &Value) -> Post {
  let post = v.at("post");
  let id = post.str("id").unwrap_or_default();
  let mut item = Post {
    url: (!id.is_empty()).then(|| post_url(&id)),
    kind: "application".into(),
    title: post.str("title"),
    created_at: time(v, "created_at"),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  for (key, path) in [
    ("application_id", "id"),
    ("status", "status"),
    ("status_label", "status_label"),
    ("company", "company_name"),
    ("source", "source_type"),
  ] {
    if let Some(value) = v.str(path) {
      item.extra.insert(key.into(), json!(value));
    }
  }
  item
}
