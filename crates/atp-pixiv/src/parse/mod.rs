//! Pixiv payloads → the shared model. Artworks (illustrations, manga,
//! ugoira) and novels are posts, in their full form (`/ajax/illust/…`,
//! `/ajax/novel/…`) or as the rows of lists (search, profiles, bookmarks,
//! the works of followed artists, rankings); users and comments are
//! users and comments ([`people`]).
//!
//! Every work carries its age rating: `extra.restriction` is `R-18` or
//! `R-18G` for restricted works ([`restricted`]), which guests never see.

mod people;
mod text;

pub use people::{comment, followed, user};

use atp_core::text::{from_secs, html_to_text, parse_time};
use atp_core::{Media, Metrics, Post, User, Value, ValueExt, json};

use crate::api::SITE;

pub fn artwork_url(id: &str) -> String {
  format!("{SITE}/artworks/{id}")
}

pub fn novel_url(id: &str) -> String {
  format!("{SITE}/novel/show.php?id={id}")
}

pub fn user_url(uid: &str) -> String {
  format!("{SITE}/users/{uid}")
}

/// Whether a work is for adults only (R-18 or R-18G).
pub fn restricted(v: &Value) -> bool {
  v.first_count(&["xRestrict", "x_restrict"])
    .is_some_and(|r| r > 0)
}

/// `illust`, `manga` or `ugoira`, from `illustType` (0, 1, 2).
fn kind(v: &Value) -> &'static str {
  match v.first_count(&["illustType", "illust_type"]) {
    Some(1) => "manga",
    Some(2) => "ugoira",
    _ => "illust",
  }
}

/// The artist of a work.
fn author(v: &Value) -> Option<User> {
  let id = v.first_str(&["userId", "user_id"]).filter(|id| id != "0")?;
  Some(User {
    name: v
      .first_str(&["userName", "user_name"])
      .unwrap_or_else(|| id.clone()),
    handle: v.str("userAccount"),
    url: Some(user_url(&id)),
    avatar: v.first_str(&["profileImageUrl", "profile_img"]),
    id,
    ..User::default()
  })
}

/// The description of a work (HTML) as text.
fn description(v: &Value) -> Option<String> {
  v.first_str(&["illustComment", "description"])
    .map(|html| html_to_text(&html))
    .filter(|text| !text.trim().is_empty())
}

/// Tags as plain strings or as `{tag}` objects (`tags.tags` in full works).
fn tags(v: &Value) -> Vec<String> {
  let list = match v.at("tags") {
    Value::Array(list) => list.as_slice(),
    other => other.list("tags"),
  };
  list
    .iter()
    .filter_map(|t| t.as_str().map(str::to_owned).or_else(|| t.str("tag")))
    .collect()
}

fn metrics(v: &Value) -> Metrics {
  Metrics {
    views: v.first_count(&["viewCount", "view_count"]),
    likes: v.first_count(&["likeCount", "rating_count"]),
    favorites: v.first_count(&["bookmarkCount", "illust_bookmark_count"]),
    comments: v.count("commentCount"),
    ..Metrics::default()
  }
}

/// The keys every work may carry: its rating, AI and original marks, its
/// series and what the account did with it.
fn marks(post: &mut Post, v: &Value) {
  let mut put = |key: &str, value: Value| {
    post.extra.insert(key.into(), value);
  };
  match v.first_count(&["xRestrict", "x_restrict"]) {
    Some(1) => put("restriction", json!("R-18")),
    Some(2) => put("restriction", json!("R-18G")),
    _ => {}
  }
  if v.u64("aiType") == Some(2) {
    put("ai_generated", json!(true));
  }
  if v.bool("isOriginal") == Some(true) || v.str("attr").as_deref() == Some("original") {
    put("original", json!(true));
  }
  if let Some(series) = series(v) {
    put("series", series);
  }
  if v.at("bookmarkData").is_object() {
    put("bookmarked", json!(true));
  }
  if v.bool("likeData") == Some(true) {
    put("liked", json!(true));
  }
  if v.first_str(&["isMasked", "is_masked"]).as_deref() == Some("true") {
    put("unavailable", json!(true));
  }
}

/// The series of a work: `seriesNavData` of full works, the series
/// fields of novels in lists, `illust_series` of rankings.
fn series(v: &Value) -> Option<Value> {
  let nav = v.at("seriesNavData");
  let ranked = v.at("illust_series");
  let id = nav
    .str("seriesId")
    .or_else(|| v.str("seriesId"))
    .or_else(|| ranked.str("illust_series_id"))?;
  Some(json!({
    "id": id,
    "title": nav
      .str("title")
      .or_else(|| v.str("seriesTitle"))
      .or_else(|| ranked.str("illust_series_title")),
    "order": nav
      .u64("order")
      .or_else(|| v.u64("seriesContentOrder"))
      .or_else(|| ranked.u64("illust_series_content_order")),
  }))
}

/// A picture in its own size, with the size the work gives.
fn image(url: String, v: &Value) -> Media {
  let mut media = Media::image(url);
  media.width = v.u64("width").map(|w| w as u32);
  media.height = v.u64("height").map(|h| h as u32);
  media.alt = v.str("alt");
  media
}

/// An artwork, full (`/ajax/illust/<id>`: the first page in original
/// size) or as a row of a list (its thumbnail).
pub fn artwork(v: &Value) -> Post {
  let id = v.first_str(&["illustId", "id"]).unwrap_or_default();
  let picture = match v.first_str(&["urls.original", "urls.regular"]) {
    Some(url) => Some(image(url, v)),
    None => v.str("url").map(Media::image),
  };
  let mut post = Post {
    kind: kind(v).into(),
    title: v.first_str(&["illustTitle", "title"]),
    text: description(v),
    url: Some(artwork_url(&id)),
    author: author(v),
    created_at: v.str("createDate").and_then(|t| parse_time(&t)),
    updated_at: v
      .first_str(&["uploadDate", "updateDate"])
      .filter(|t| Some(t) != v.str("createDate").as_ref())
      .and_then(|t| parse_time(&t)),
    metrics: metrics(v),
    media: picture.into_iter().collect(),
    tags: tags(v),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  if let Some(pages) = v.count("pageCount") {
    post.extra.insert("pages".into(), json!(pages));
  }
  marks(&mut post, v);
  post
}

/// The pages of an artwork (`/ajax/illust/<id>/pages`), in original size.
pub fn pages(list: &[Value]) -> Vec<Media> {
  list
    .iter()
    .filter_map(|page| {
      page
        .first_str(&["urls.original", "urls.regular"])
        .map(|url| image(url, page))
    })
    .collect()
}

/// An entry of a ranking (`ranking.php?format=json`).
pub fn ranked(v: &Value) -> Post {
  let id = v.str("illust_id").unwrap_or_default();
  let mut post = Post {
    kind: kind(v).into(),
    title: v.str("title"),
    url: Some(artwork_url(&id)),
    author: author(v),
    created_at: v.i64("illust_upload_timestamp").and_then(from_secs),
    metrics: metrics(v),
    media: v.str("url").map(Media::image).into_iter().collect(),
    tags: tags(v),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let fields = [
    ("rank", v.u64("rank")),
    ("previous_rank", v.u64("yes_rank").filter(|r| *r > 0)),
    ("pages", v.u64("illust_page_count")),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      post.extra.insert(key.into(), json!(value));
    }
  }
  marks(&mut post, v);
  post
}

/// A novel, full (`/ajax/novel/<id>`: its text) or as a row of a list.
pub fn novel(v: &Value) -> Post {
  let id = v.str("id").unwrap_or_default();
  let body = v.str("content").map(|content| text::novel(&content));
  let mut media: Vec<Media> = v
    .first_str(&["coverUrl", "url"])
    .map(Media::image)
    .into_iter()
    .collect();
  if let Some(images) = v.at("textEmbeddedImages").as_object() {
    media.extend(
      images
        .values()
        .filter_map(|i| i.first_str(&["urls.original", "urls.1200x1200"]))
        .map(Media::image),
    );
  }
  let mut post = Post {
    kind: "novel".into(),
    title: v.str("title"),
    text: body.or_else(|| description(v)),
    url: Some(novel_url(&id)),
    author: author(v),
    created_at: v.str("createDate").and_then(|t| parse_time(&t)),
    updated_at: v
      .first_str(&["uploadDate", "updateDate"])
      .filter(|t| Some(t) != v.str("createDate").as_ref())
      .and_then(|t| parse_time(&t)),
    metrics: metrics(v),
    media,
    tags: tags(v),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let fields = [
    ("words", v.count("wordCount")),
    (
      "characters",
      v.first_count(&["characterCount", "textCount"]),
    ),
    ("reading_seconds", v.count("readingTime")),
  ];
  for (key, value) in fields {
    if let Some(value) = value.filter(|n| *n > 0) {
      post.extra.insert(key.into(), json!(value));
    }
  }
  if v.str("content").is_some()
    && let Some(about) = description(v)
  {
    post.extra.insert("description".into(), json!(about));
  }
  if let Some(language) = v.str("language") {
    post.extra.insert("language".into(), json!(language));
  }
  marks(&mut post, v);
  post
}
