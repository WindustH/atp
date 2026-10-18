//! Steam payloads → the shared model. Apps (games, DLC, software) are
//! posts with their price, reviews are comments, profiles are users and
//! owned games are posts again. Store prices come in cents; texts of app
//! pages are HTML, reviews use Steam's BBCode-like markup. Web API
//! profiles and games are in [`profiles`].

mod profiles;

pub use profiles::{owned, player};

use std::sync::LazyLock;

use atp_core::text::{date_only, from_secs, html_to_text};
use atp_core::{Comment, Media, Metrics, Post, Price, User, UserStats, Value, ValueExt, json};
use jiff::civil::Date;
use regex::Regex;

use crate::api::{COMMUNITY, STORE};

/// Formats of `release_date.date` across store languages.
const RELEASE_FORMATS: &[&str] = &[
  "%d %b, %Y",
  "%b %d, %Y",
  "%d %B, %Y",
  "%B %d, %Y",
  "%Y年%m月%d日",
  "%Y-%m-%d",
];

static LINK: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?s)\[url=([^\]]+)\](.*?)\[/url\]").unwrap());
static TAG: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\[/?[a-z*][a-z0-9]*(?:=[^\]]*)?\]").unwrap());

pub fn app_url(id: &str) -> String {
  format!("{STORE}/app/{id}/")
}

pub fn profile_url(steam_id: &str) -> String {
  format!("{COMMUNITY}/profiles/{steam_id}")
}

/// A price from amounts in cents; the crossed-out one only when it differs.
fn price(currency: Option<String>, cents: Option<u64>, before: Option<u64>) -> Option<Price> {
  let amount = cents? as f64 / 100.0;
  let original = before.map(|c| c as f64 / 100.0).filter(|o| *o > amount);
  let discount = original.map(|o| format!("-{:.0}%", (1.0 - amount / o) * 100.0));
  Some(Price {
    amount,
    currency: currency.filter(|c| !c.is_empty())?,
    original,
    label: discount,
  })
}

/// The systems an app runs on, from `<prefix>windows` … flags.
fn platforms(v: &Value, prefix: &str, suffix: &str) -> Vec<Value> {
  ["windows", "mac", "linux"]
    .into_iter()
    .filter(|p| v.bool(&format!("{prefix}{p}{suffix}")) == Some(true))
    .map(Value::from)
    .collect()
}

/// The names in a list (of strings, or of objects under `key`), each once.
fn names(v: &Value, path: &str, key: &str) -> Vec<String> {
  let mut names: Vec<String> = Vec::new();
  for item in v.list(path) {
    let name = match item {
      Value::String(s) => Some(s.clone()),
      item => item.str(key),
    };
    if let Some(name) = name.filter(|n| !names.contains(n)) {
      names.push(name);
    }
  }
  names
}

fn release(v: &Value) -> Option<jiff::Timestamp> {
  let shown = v.str("release_date.date")?;
  let compact: String = shown.chars().filter(|c| !c.is_whitespace()).collect();
  RELEASE_FORMATS
    .iter()
    .find_map(|format| {
      Date::strptime(format, &shown)
        .or_else(|_| Date::strptime(format, &compact))
        .ok()
    })
    .and_then(date_only)
}

fn html(v: &Value, path: &str) -> Option<String> {
  v.str(path)
    .map(|t| html_to_text(&t))
    .filter(|t| !t.is_empty())
}

/// An app page (`appdetails` → `data`), with the review summary
/// (`appreviews` → `query_summary`) when it was loaded.
pub fn app(v: &Value, reviews: Option<&Value>) -> Post {
  let id = v.str("steam_appid").unwrap_or_default();
  let text: Vec<String> = [
    html(v, "short_description"),
    html(v, "detailed_description"),
  ]
  .into_iter()
  .flatten()
  .collect();
  let mut post = Post {
    kind: v.str("type").unwrap_or_else(|| "app".into()),
    title: v.str("name"),
    text: Some(text.join("\n\n")).filter(|t| !t.is_empty()),
    url: Some(app_url(&id)),
    author: v.str("developers.0").map(|name| User {
      id: name.clone(),
      name,
      ..User::default()
    }),
    created_at: release(v),
    price: price(
      v.str("price_overview.currency"),
      v.u64("price_overview.final"),
      v.u64("price_overview.initial"),
    ),
    media: media(v),
    tags: names(v, "genres", "description"),
    metrics: Metrics {
      likes: reviews.and_then(|r| r.u64("total_positive")),
      comments: reviews.and_then(|r| r.u64("total_reviews")),
      ..Metrics::default()
    },
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  app_extra(&mut post, v, reviews);
  post
}

fn app_extra(post: &mut Post, v: &Value, reviews: Option<&Value>) {
  let extra = &mut post.extra;
  let mut put = |key: &str, value: Option<Value>| {
    if let Some(value) = value.filter(|v| !v.is_null()) {
      extra.insert(key.into(), value);
    }
  };
  put("free", v.bool("is_free").filter(|f| *f).map(Value::from));
  put("developers", Some(names(v, "developers", "").into()));
  put("publishers", Some(names(v, "publishers", "").into()));
  put("platforms", Some(platforms(v, "platforms.", "").into()));
  put(
    "categories",
    Some(names(v, "categories", "description").into()),
  );
  put("release_date", v.str("release_date.date").map(Value::from));
  let soon = v.bool("release_date.coming_soon").filter(|s| *s);
  put("coming_soon", soon.map(Value::from));
  put("metacritic", v.u64("metacritic.score").map(Value::from));
  put("website", v.str("website").map(Value::from));
  let age = v.u64("required_age").filter(|a| *a > 0);
  put("required_age", age.map(Value::from));
  let languages = html(v, "supported_languages").map(|l| l.replace('\n', " "));
  put("languages", languages.map(Value::from));
  put("achievements", v.u64("achievements.total").map(Value::from));
  put(
    "recommendations",
    v.u64("recommendations.total").map(Value::from),
  );
  let dlc = v.list("dlc").len();
  put("dlc", (dlc > 0).then(|| dlc.into()));
  put("reviews", reviews.map(review_summary));
  extra.retain(|_, value| value.as_array().is_none_or(|a| !a.is_empty()));
}

/// `Very Positive`, the counts and the share of positive reviews.
pub fn review_summary(v: &Value) -> Value {
  let total = v.u64("total_reviews").unwrap_or_default();
  let positive = v.u64("total_positive").unwrap_or_default();
  let percent = (total > 0).then(|| (positive * 100 + total / 2) / total);
  json!({
    "summary": v.str("review_score_desc"),
    "positive": positive,
    "negative": v.u64("total_negative"),
    "total": total,
    "percent": percent,
  })
}

/// The header picture, screenshots and trailers that have a file link.
fn media(v: &Value) -> Vec<Media> {
  let mut media: Vec<Media> = v
    .str("header_image")
    .map(Media::image)
    .into_iter()
    .collect();
  media.extend(
    v.list("screenshots")
      .iter()
      .filter_map(|s| s.str("path_full"))
      .map(Media::image),
  );
  media.extend(v.list("movies").iter().filter_map(|m| {
    let url = m.first_str(&["mp4.max", "webm.max", "mp4.480", "webm.480"])?;
    let mut video = Media::video(url);
    video.alt = m.str("name");
    Some(video)
  }));
  media
}

/// A `storesearch` item.
pub fn search_item(v: &Value) -> Post {
  let id = v.str("id").unwrap_or_default();
  let mut post = Post {
    kind: "app".into(),
    title: v.str("name"),
    url: Some(app_url(&id)),
    price: price(
      v.str("price.currency"),
      v.u64("price.final"),
      v.u64("price.initial"),
    ),
    media: v.str("tiny_image").map(Media::image).into_iter().collect(),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let systems = platforms(v, "platforms.", "");
  if !systems.is_empty() {
    post.extra.insert("platforms".into(), systems.into());
  }
  if let Some(score) = v.u64("metascore") {
    post.extra.insert("metacritic".into(), score.into());
  }
  post
}

/// An item of a `featuredcategories` list.
pub fn featured(v: &Value) -> Post {
  let id = v.str("id").unwrap_or_default();
  let mut post = Post {
    kind: "app".into(),
    title: v.str("name"),
    url: Some(app_url(&id)),
    price: price(
      v.str("currency"),
      v.u64("final_price"),
      v.u64("original_price"),
    ),
    media: v
      .first_str(&["header_image", "large_capsule_image"])
      .map(Media::image)
      .into_iter()
      .collect(),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let systems = platforms(v, "", "_available");
  if !systems.is_empty() {
    post.extra.insert("platforms".into(), systems.into());
  }
  let ends = v.i64("discount_expiration").and_then(from_secs);
  if let Some(ends) = ends.filter(|_| v.u64("discount_percent").unwrap_or(0) > 0) {
    post
      .extra
      .insert("discount_ends".into(), ends.to_string().into());
  }
  post
}

/// Review markup as plain text: `[url=…]text[/url]` keeps its text and
/// link, other tags (`[b]`, `[h1]`, `[spoiler]` …) go.
fn markup(text: &str) -> String {
  let linked = LINK.replace_all(text, "$2 ($1)");
  TAG.replace_all(&linked, "").trim().to_owned()
}

fn hours(minutes: Option<u64>) -> Option<Value> {
  minutes.map(|m| json!((m as f64 / 6.0).round() / 10.0))
}

/// A review (`appreviews` → `reviews`); its author is known by SteamID only.
pub fn review(v: &Value, app: &str) -> Comment {
  let steam_id = v.str("author.steamid").unwrap_or_default();
  let author = User {
    url: Some(profile_url(&steam_id)),
    stats: UserStats {
      posts: v.u64("author.num_reviews"),
      other: v
        .u64("author.num_games_owned")
        .map(|n| [("games".to_owned(), n)].into())
        .unwrap_or_default(),
      ..UserStats::default()
    },
    id: steam_id.clone(),
    name: steam_id.clone(),
    ..User::default()
  };
  let mut comment = Comment {
    id: v.str("recommendationid").unwrap_or_default(),
    author: Some(author),
    text: markup(&v.str("review").unwrap_or_default()),
    created_at: v.i64("timestamp_created").and_then(from_secs),
    likes: v.u64("votes_up"),
    reply_count: v.u64("comment_count"),
    raw: Some(v.clone()),
    ..Comment::default()
  };
  let url = format!("{}/recommended/{app}/", profile_url(&steam_id));
  comment.extra.insert("url".into(), json!(url));
  review_extra(&mut comment, v);
  comment
}

/// What a review tells beyond the shared fields: the verdict, funny
/// votes, language, hours played, how the game was had, a later change.
fn review_extra(comment: &mut Comment, v: &Value) {
  let extra = &mut comment.extra;
  extra.insert("recommended".into(), json!(v.bool("voted_up")));
  let fields = [
    ("funny", v.u64("votes_funny").map(Value::from)),
    ("language", v.str("language").map(Value::from)),
    ("hours_at_review", hours(v.u64("author.playtime_at_review"))),
    ("hours_total", hours(v.u64("author.playtime_forever"))),
    ("steam_purchase", v.bool("steam_purchase").map(Value::from)),
    (
      "free",
      v.bool("received_for_free").filter(|f| *f).map(Value::from),
    ),
    (
      "early_access",
      v.bool("written_during_early_access")
        .filter(|e| *e)
        .map(Value::from),
    ),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      extra.insert(key.into(), value);
    }
  }
  let updated = v.i64("timestamp_updated").and_then(from_secs);
  if let Some(updated) = updated.filter(|u| Some(*u) != comment.created_at) {
    comment
      .extra
      .insert("updated_at".into(), json!(updated.to_string()));
  }
}
