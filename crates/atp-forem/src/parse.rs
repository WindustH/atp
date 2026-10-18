//! Forem payloads → the shared model. Articles are posts (their Markdown
//! as text when the answer has it), comments are comments with their
//! replies, users are users and tags are collections. Listings and single
//! articles swap `tag_list` and `tags` (an array in one, a string in the
//! other); both are read.

use atp_core::text::{date_only, html_to_text, parse_time};
use atp_core::{Collection, Comment, Media, Metrics, Post, User, Value, ValueExt, json};

/// The tags of an article, from whichever field is an array, else from
/// the comma-separated one.
fn tags(v: &Value) -> Vec<String> {
  for path in ["tag_list", "tags"] {
    if let Some(list) = v.at(path).as_array() {
      return list
        .iter()
        .filter_map(|t| t.as_str().map(str::to_owned))
        .collect();
    }
  }
  v.first_str(&["tag_list", "tags"])
    .map(|s| {
      s.split(',')
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect()
    })
    .unwrap_or_default()
}

/// The author of an article or comment (`user`), by username.
pub fn author(v: &Value, instance: &str) -> Option<User> {
  let name = v.str("username")?;
  Some(User {
    name: v.str("name").unwrap_or_else(|| name.clone()),
    handle: Some(name.clone()),
    url: Some(format!("{instance}/{name}")),
    avatar: v.first_str(&["profile_image_90", "profile_image"]),
    id: name,
    ..User::default()
  })
}

/// An article of a listing or a single answer.
pub fn article(v: &Value, instance: &str) -> Post {
  let id = v.str("id").unwrap_or_default();
  let mut post = Post {
    kind: "article".into(),
    title: v.str("title"),
    text: v.first_str(&["body_markdown", "description"]),
    url: v
      .str("url")
      .or_else(|| v.str("path").map(|p| format!("{instance}{p}"))),
    author: author(v.at("user"), instance),
    created_at: v
      .first_str(&["published_timestamp", "published_at", "created_at"])
      .and_then(|t| parse_time(&t)),
    updated_at: v.str("edited_at").and_then(|t| parse_time(&t)),
    metrics: Metrics {
      likes: v.first_count(&["public_reactions_count", "positive_reactions_count"]),
      comments: v.count("comments_count"),
      views: v.count("page_views_count"),
      ..Metrics::default()
    },
    media: v.str("cover_image").map(Media::image).into_iter().collect(),
    tags: tags(v),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let fields = [
    (
      "description",
      v.str("description")
        .filter(|_| v.str("body_markdown").is_some()),
    ),
    ("organization", v.str("organization.name")),
    (
      "canonical_url",
      v.str("canonical_url")
        .filter(|c| Some(c) != post.url.as_ref()),
    ),
    ("slug", v.str("slug")),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      post.extra.insert(key.into(), json!(value));
    }
  }
  if let Some(minutes) = v.u64("reading_time_minutes") {
    post.extra.insert("reading_minutes".into(), json!(minutes));
  }
  if v.bool("published") == Some(false) {
    post.extra.insert("draft".into(), json!(true));
  }
  post
}

/// A comment with every reply under it.
pub fn comment(v: &Value, instance: &str) -> Comment {
  let replies: Vec<Comment> = v
    .list("children")
    .iter()
    .map(|child| comment(child, instance))
    .collect();
  let id = v.str("id_code").unwrap_or_default();
  let author = author(v.at("user"), instance);
  let url = author
    .as_ref()
    .map(|a| format!("{instance}/{}/comment/{id}", a.id));
  let raw: serde_json::Map<String, Value> = v
    .as_object()
    .into_iter()
    .flatten()
    .filter(|(key, _)| *key != "children")
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect();
  Comment {
    author,
    text: v
      .str("body_html")
      .map(|h| html_to_text(&h))
      .unwrap_or_default(),
    created_at: v.str("created_at").and_then(|t| parse_time(&t)),
    reply_count: Some(replies.len() as u64),
    replies,
    extra: url
      .map(|u| [("url".to_owned(), json!(u))].into())
      .unwrap_or_default(),
    raw: Some(Value::Object(raw)),
    id,
    ..Comment::default()
  }
}

/// A profile (`/api/users/…`).
pub fn user(v: &Value, instance: &str) -> User {
  let name = v.str("username").unwrap_or_default();
  let joined = v
    .str("joined_at")
    .and_then(|d| jiff::civil::Date::strptime("%b %d, %Y", d.trim()).ok())
    .and_then(date_only);
  let mut user = User {
    name: v.str("name").unwrap_or_else(|| name.clone()),
    handle: Some(name.clone()),
    url: Some(format!("{instance}/{name}")),
    avatar: v.str("profile_image"),
    bio: v.str("summary"),
    location: v.str("location"),
    created_at: joined,
    raw: Some(v.clone()),
    id: name,
    ..User::default()
  };
  let fields = [
    ("user_id", v.str("id")),
    ("website", v.str("website_url")),
    ("github", v.str("github_username")),
    ("twitter", v.str("twitter_username")),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      user.extra.insert(key.into(), json!(value));
    }
  }
  user
}

/// A tag of `/api/tags`.
pub fn tag(v: &Value, instance: &str) -> Collection {
  let name = v.str("name").unwrap_or_default();
  let mut tag = Collection {
    kind: "tag".into(),
    url: Some(format!("{instance}/t/{name}")),
    description: v.str("short_summary"),
    raw: Some(v.clone()),
    id: name.clone(),
    name: format!("#{name}"),
    ..Collection::default()
  };
  if let Some(color) = v.str("bg_color_hex") {
    tag.extra.insert("color".into(), json!(color));
  }
  tag
}
