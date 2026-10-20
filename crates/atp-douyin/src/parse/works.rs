//! The creator platform's own lists: the works list (`creator/item/list`)
//! with each work's metrics, review state and visibility, and the drafts.
//!
//! The works list sends ids as JSON numbers; they are read as integers
//! (exact up to `u64`), never through a float.

use std::collections::BTreeMap;

use atp_core::text::from_unix;
use atp_core::{Extra, Insights, Metrics, Page, Post, Value, ValueExt, json};

use super::put;
use crate::refs::{note_url, video_url};

/// The metrics of a work as the creator pages name them, the names ATP
/// gives them, and whether they are rates. The page shows a rate of at
/// most 1 as a fraction and a larger one as a percentage; ATP makes both
/// 0..1 fractions.
const METRICS: &[(&str, &str, bool)] = &[
  ("view_count", "views", false),
  ("like_count", "likes", false),
  ("comment_count", "comments", false),
  ("share_count", "shares", false),
  ("favorite_count", "favorites", false),
  ("danmaku_count", "danmaku", false),
  ("subscribe_count", "new_followers", false),
  ("cover_show", "cover_impressions", false),
  ("avg_view_second", "avg_watch_seconds", false),
  ("image_avg_view_count", "avg_images_viewed", false),
  ("completion_rate", "completion_rate", true),
  ("completion_rate_5s", "five_second_completion_rate", true),
  ("bounce_rate_2s", "two_second_bounce_rate", true),
  ("avg_view_proportion", "avg_watch_ratio", true),
  ("cover_click_rate", "cover_ctr", true),
  ("description_spread_rate", "text_expand_rate", true),
  ("enter_detail_ratio", "detail_open_rate", true),
  ("long_article_finish_ratio", "read_completion_rate", true),
];

/// `ItemTypeEnum` of the creator item service: 1 pictures, 2 to 5
/// videos by length, 6 and 7 slides, 8 articles.
const PICTURES: i64 = 1;
const ARTICLE: i64 = 8;

/// The id of a works-list row.
pub fn work_id(v: &Value) -> Option<String> {
  v.first_str(&["id", "item_id", "aweme_id"])
}

/// A page of the works list: its `items`, and `max_cursor` while
/// `has_more` holds and the cursor moves on from `cursor`.
pub fn works(v: &Value, cursor: Option<&str>) -> Page<Post> {
  let items: Vec<Post> = v.list("items").iter().filter_map(work).collect();
  let next = works_cursor(v, cursor).filter(|_| !items.is_empty());
  Page::new(items, next)
}

/// The cursor of the works list's next page.
pub fn works_cursor(v: &Value, cursor: Option<&str>) -> Option<String> {
  let more = v.bool("has_more") == Some(true);
  v.str("max_cursor")
    .filter(|next| more && next != "0" && Some(next.as_str()) != cursor)
}

/// A work of the works list: the counters in `metrics`, the other creator
/// metrics, `review`, `visibility` and `type` in `extra`.
pub fn work(v: &Value) -> Option<Post> {
  let id = work_id(v)?;
  let mut metrics = Metrics::default();
  let mut extra = Extra::new();
  for (name, value) in work_stats(v.at("metrics")) {
    let count = value.as_u64();
    match (name.as_str(), count) {
      ("views", Some(n)) => metrics.views = Some(n),
      ("likes", Some(n)) => metrics.likes = Some(n),
      ("comments", Some(n)) => metrics.comments = Some(n),
      ("shares", Some(n)) => metrics.shares = Some(n),
      ("favorites", Some(n)) => metrics.favorites = Some(n),
      ("danmaku" | "new_followers" | "cover_impressions", Some(n)) => {
        metrics.other.insert(name, n);
      }
      _ => {
        extra.insert(name, value);
      }
    }
  }
  states(&mut extra, v);
  let kind = v.i64("type");
  let desc = v.first_str(&["description", "desc", "text"]);
  Some(Post {
    url: match kind {
      Some(ARTICLE) => None,
      Some(PICTURES) => Some(note_url(&id)),
      _ => Some(video_url(&id)),
    },
    kind: match kind {
      Some(ARTICLE) => "article",
      Some(PICTURES) => "note",
      _ => "video",
    }
    .into(),
    title: v
      .first_str(&["item_title", "title"])
      .filter(|t| Some(t) != desc.as_ref()),
    text: desc,
    created_at: ["create_time", "publish_time"]
      .iter()
      .find_map(|k| v.i64(k))
      .and_then(from_unix),
    metrics,
    extra,
    raw: Some(v.clone()),
    id,
    ..Post::default()
  })
}

/// The insights of one work: its metrics as totals, as the works list
/// gives them (no window: the work's numbers so far).
pub fn work_insights(v: &Value) -> Option<Insights> {
  let post = work(v)?;
  let mut insights = Insights {
    kind: "post".into(),
    subject: post.id.clone(),
    title: post.title.clone().or_else(|| post.text.clone()),
    url: post.url.clone(),
    totals_period: Some("lifetime".into()),
    raw: Some(v.clone()),
    ..Insights::default()
  };
  for (name, value) in work_stats(v.at("metrics")) {
    insights.total(&name, value);
  }
  if insights.totals.is_empty() {
    insights
      .warnings
      .push("Douyin gave no creator metrics for this work".into());
  }
  states(&mut insights.extra, v);
  insights.extra.insert("type".into(), json!(post.kind));
  Some(insights)
}

/// A work's metrics under ATP's names: counts as integers, rates as 0..1
/// fractions; metrics ATP does not know keep their own names and values.
pub fn work_stats(metrics: &Value) -> BTreeMap<String, Value> {
  let mut stats = BTreeMap::new();
  let Some(fields) = metrics.as_object() else {
    return stats;
  };
  for (key, value) in fields {
    let Some(n) = number(value) else { continue };
    let known = METRICS.iter().find(|(k, ..)| k == key);
    let (name, rate) = known.map_or((key.as_str(), false), |(_, name, rate)| (*name, *rate));
    let value = if rate {
      json!(if n.abs() > 1.0 { n / 100.0 } else { n })
    } else if n.fract() == 0.0 && n.abs() < 9e15 {
      json!(n as i64)
    } else {
      json!(n)
    };
    stats.insert(name.to_owned(), value);
  }
  stats
}

/// A number, or a string holding one (the list sends most as strings).
fn number(v: &Value) -> Option<f64> {
  let n = match v {
    Value::Number(n) => n.as_f64(),
    Value::String(s) => s.trim().parse().ok(),
    _ => None,
  }?;
  n.is_finite().then_some(n)
}

/// `review` and `visibility` of a works-list row.
fn states(extra: &mut Extra, v: &Value) {
  put(extra, "review", review(v).map(Value::from));
  put(extra, "visibility", visibility(v).map(Value::from));
}

/// The review state, by `ReviewStatusEnum` of the creator item service.
fn review(v: &Value) -> Option<String> {
  let paths = [
    "review.status",
    "review.review_status",
    "review_status",
    "review",
  ];
  let code = paths.iter().find_map(|p| v.i64(p))?;
  let name = match code {
    0 => return None,
    1 => "reviewing",
    2 => "passed",
    3 => "self_only",
    4 => "rejected",
    5 => "advice",
    6 => "friends_only",
    other => return Some(format!("code {other}")),
  };
  Some(name.into())
}

/// Who may see the work, by `VisibilityEnum` of the creator item service.
fn visibility(v: &Value) -> Option<String> {
  let paths = [
    "visibility.visibility",
    "visibility.type",
    "visibility.visibility_type",
    "visibility.status",
    "visibility",
  ];
  let code = paths.iter().find_map(|p| v.i64(p))?;
  let name = match code {
    0 => return None,
    1 => "public",
    2 => "friends",
    3 => "private",
    other => return Some(format!("code {other}")),
  };
  Some(name.into())
}

/// A draft (草稿): not published, so without a link.
pub fn draft(v: &Value) -> Option<Post> {
  let id = v.first_str(&["aweme_id", "item_id", "creation_id"])?;
  let desc = v.first_str(&["desc", "description", "text"]);
  Some(Post {
    kind: "draft".into(),
    title: v
      .first_str(&["item_title", "title"])
      .filter(|t| Some(t) != desc.as_ref()),
    text: desc,
    created_at: v.i64("create_time").and_then(from_unix),
    updated_at: v.i64("update_time").and_then(from_unix),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  })
}
