//! Nowcoder payloads → the shared model: jobs, schedule entries and
//! applications ([`jobs`]); community posts, moments (动态) and long posts
//! (帖子), from feeds, search and their pages; comments; users; hot
//! searches and topics.
//!
//! What the site tells about people beyond their name, avatar and level is
//! dropped wherever it appears, `raw` included ([`sanitize`]): IP addresses,
//! schools, majors, work years, employer identities, e-mail addresses; and
//! the recruiters of jobs ([`sanitize_job`]).

mod jobs;
mod schedule;

pub use jobs::{application, job, saved_job};
pub use schedule::schedule;

use atp_core::text::{from_millis, html_to_text};
use atp_core::{Comment, Media, Metrics, Post, User, Value, ValueExt, json};
use jiff::{Timestamp, ToSpan};

use crate::refs;

/// Keys of what the site tells about people, dropped wherever they appear.
const PRIVATE_KEYS: &[&str] = &[
  "ip4",
  "educationInfo",
  "secondMajorName",
  "schoolMajor",
  "authDisplayInfo",
  "workTime",
  "workStatusDetail",
  "identityList",
  "identity",
  "enterpriseInfo",
  "livePlace",
  "gender",
  "email",
  "relateEmail",
  "phone",
  "mobile",
];

/// The recruiters of a job: dropped from job payloads as well.
const RECRUITER_KEYS: &[&str] = &[
  "user",
  "bossUid",
  "apiSimpleBossUser",
  "jobPublisher",
  "hrIdentityInfo",
  "greeting",
];

/// Content types of the community.
pub const MOMENT: i64 = 74;
pub const CONTENT: i64 = 250;

fn strip(v: &Value, drop: &dyn Fn(&str) -> bool) -> Value {
  match v {
    Value::Object(map) => Value::Object(
      map
        .iter()
        .filter(|(key, _)| !drop(key))
        .map(|(key, value)| (key.clone(), strip(value, drop)))
        .collect(),
    ),
    Value::Array(items) => Value::Array(items.iter().map(|i| strip(i, drop)).collect()),
    other => other.clone(),
  }
}

/// The payload without what it tells about people.
pub fn sanitize(v: &Value) -> Value {
  strip(v, &|key| PRIVATE_KEYS.contains(&key))
}

/// A job payload without its recruiters and what it tells about people.
pub fn sanitize_job(v: &Value) -> Value {
  strip(v, &|key| {
    PRIVATE_KEYS.contains(&key) || RECRUITER_KEYS.contains(&key)
  })
}

/// A time in milliseconds.
pub fn millis(v: &Value, path: &str) -> Option<Timestamp> {
  v.i64(path).filter(|n| *n > 0).and_then(from_millis)
}

/// A time in milliseconds as the day it falls on in Beijing (deadlines,
/// application dates), unless it is decades away: jobs open without an
/// end say 2122.
pub fn day(v: &Value, path: &str) -> Option<String> {
  let at = millis(v, path)?;
  let far = Timestamp::now().checked_add((24 * 365 * 10).hours()).ok()?;
  let beijing = jiff::tz::TimeZone::fixed(jiff::tz::offset(8));
  (at < far).then(|| at.to_zoned(beijing).date().to_string())
}

/// A user as posts and comments show them (`userBrief`): name, avatar, level.
pub fn user(v: &Value) -> Option<User> {
  let id = v.first_str(&["userId", "id"]).filter(|id| id != "0")?;
  let mut user = User {
    url: Some(refs::user_url(&id)),
    name: v.str("nickname").unwrap_or_else(|| id.clone()),
    avatar: v.first_str(&["headImgUrl", "headUrl"]),
    bio: v.str("introduction").filter(|b| !b.trim().is_empty()),
    followed: v.bool("followed"),
    id,
    ..User::default()
  };
  if let Some(level) = v.str("honorLevelName") {
    user.extra.insert("level".into(), json!(level));
  }
  Some(user)
}

/// Text that may be HTML (long posts, some snippets).
fn text(s: &str) -> String {
  if s.contains('<') {
    html_to_text(s)
  } else {
    s.trim().to_owned()
  }
}

/// Counters of `frequencyData`; `followCnt` counts saves (收藏).
fn metrics(v: &Value) -> Metrics {
  let f = v.at("frequencyData");
  Metrics {
    views: f.u64("viewCnt"),
    likes: f.u64("likeCnt"),
    comments: f.u64("totalCommentCnt").or_else(|| f.u64("commentCnt")),
    shares: f.u64("shareCnt"),
    favorites: f.u64("followCnt"),
    ..Metrics::default()
  }
}

/// The topics (话题) a post is filed under.
fn topics(v: &Value) -> Vec<String> {
  v.list("subjectData")
    .iter()
    .filter_map(|s| s.str("content"))
    .map(|s| s.trim_matches('#').to_owned())
    .filter(|s| !s.is_empty())
    .collect()
}

/// A moment (动态): its data and the card around it (author, counters).
fn moment(m: &Value, card: &Value) -> Option<Post> {
  let uuid = m.str("uuid")?;
  let media = m
    .list("imgMoment")
    .iter()
    .filter_map(|img| {
      let mut media = Media::image(img.str("src")?);
      media.width = img.u64("width").and_then(|w| u32::try_from(w).ok());
      media.height = img.u64("height").and_then(|h| u32::try_from(h).ok());
      Some(media)
    })
    .collect();
  let mut post = Post {
    url: Some(refs::moment_url(&uuid)),
    kind: "moment".into(),
    title: m.str("title").filter(|t| !t.trim().is_empty()),
    text: m.str("content").map(|c| text(&c)),
    author: user(card.at("userBrief")),
    created_at: millis(m, "createdAt").or_else(|| millis(m, "showTime")),
    metrics: metrics(card),
    media,
    tags: topics(card),
    id: uuid,
    ..Post::default()
  };
  if let Some(id) = m.str("id") {
    post.extra.insert("entity_id".into(), json!(id));
  }
  Some(post)
}

/// A long post (帖子): its data (the whole `richText` on its page) and the
/// card around it.
fn content(c: &Value, card: &Value) -> Option<Post> {
  let id = c.str("id")?;
  let body = c.str("richText").or_else(|| c.str("content"));
  Some(Post {
    url: Some(refs::content_url(&id)),
    kind: "post".into(),
    title: c.str("title").filter(|t| !t.trim().is_empty()),
    text: body.map(|b| text(&b)),
    author: user(card.at("userBrief")),
    created_at: millis(c, "createTime").or_else(|| millis(c, "showTime")),
    updated_at: millis(c, "editTime").filter(|_| c.bool("edited") == Some(true)),
    metrics: metrics(card),
    tags: topics(card),
    id,
    ..Post::default()
  })
}

/// A post of a feed, a search or a list (`{contentType, momentData |
/// contentData, userBrief, frequencyData}`, some wrapped in `data`);
/// `None` for ads, users and other cards.
pub fn record(v: &Value) -> Option<Post> {
  let v = if v.at("contentType").is_null() && v.at("data").is_object() {
    v.at("data")
  } else {
    v
  };
  let mut post = match v.i64("contentType")? {
    MOMENT => moment(v.at("momentData"), v)?,
    CONTENT => content(v.at("contentData"), v)?,
    _ => return None,
  };
  post.raw = Some(sanitize(v));
  Some(post)
}

/// A post's page data (`detail/moment-data/detail/<uuid>`,
/// `detail/content-data/detail/<id>`): the data and its card in one.
pub fn detail(v: &Value, kind: i64) -> Option<Post> {
  let mut post = if kind == MOMENT {
    moment(v, v)?
  } else {
    content(v, v)?
  };
  if let Some(place) = v.str("ip4Location") {
    post.extra.insert("location".into(), json!(place));
  }
  post.raw = Some(sanitize(v));
  Some(post)
}

/// A comment or a reply (`comment/list-by-page`).
pub fn comment(v: &Value) -> Comment {
  let reply_to = (v.u64("toCommentId").unwrap_or(0) != 0)
    .then(|| v.str("toUserBrief.nickname"))
    .flatten();
  let body = v
    .str("pureText")
    .or_else(|| v.str("content").map(|c| text(&c)))
    .unwrap_or_default();
  Comment {
    id: v.str("id").unwrap_or_default(),
    author: user(v.at("userBrief")),
    text: body,
    created_at: millis(v, "createTime"),
    likes: v.u64("frequencyData.likeCnt"),
    reply_count: v.u64("frequencyData.commentCnt"),
    reply_to,
    location: v.str("ip4Location"),
    raw: Some(sanitize(v)),
    ..Comment::default()
  }
}

/// An entry of the hot posts (热门帖子, `hot-search/top-hot-pc`).
pub fn hot_post(v: &Value) -> Option<Post> {
  let (kind, url, id) = match v.i64("type") {
    Some(CONTENT) => {
      let id = v.str("id")?;
      ("post", refs::content_url(&id), id)
    }
    _ => {
      let uuid = v.str("uuid")?;
      ("moment", refs::moment_url(&uuid), uuid)
    }
  };
  let mut post = Post {
    kind: kind.into(),
    title: v.str("title"),
    url: Some(url),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  if let Some(heat) = v.u64("hotValueFromDolphin") {
    post.metrics.other.insert("heat".into(), heat);
  }
  if kind == "moment"
    && let Some(id) = v.str("id")
  {
    post.extra.insert("entity_id".into(), json!(id));
  }
  Some(post)
}

/// A hot search (热搜, `hot-search/hot-content`): the words, searched on
/// the site by its link.
pub fn trend(v: &Value) -> Option<Post> {
  let query = v.str("query")?;
  let link = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
  let mut post = Post {
    id: query.clone(),
    kind: "trend".into(),
    url: Some(format!(
      "{}/search/all?query={link}&type=all",
      crate::api::WWW
    )),
    title: Some(query),
    raw: Some(v.clone()),
    ..Post::default()
  };
  if let Some(heat) = v.u64("hotValue") {
    post.metrics.other.insert("heat".into(), heat);
  }
  if let Some(rank) = v.u64("rank") {
    post.extra.insert("rank".into(), json!(rank));
  }
  Some(post)
}

/// A hot topic (话题, `subject/hot-subject`).
pub fn topic(v: &Value) -> Option<Post> {
  let uuid = v.str("uuid")?;
  let mut post = Post {
    url: Some(format!("{}/subject/index/{uuid}", crate::api::WWW)),
    kind: "topic".into(),
    title: v.str("content"),
    metrics: Metrics {
      views: v.u64("viewCount"),
      ..Metrics::default()
    },
    raw: Some(v.clone()),
    id: uuid,
    ..Post::default()
  };
  for (key, path) in [("posts", "momentCount"), ("heat", "hotValue")] {
    if let Some(n) = v.u64(path) {
      post.metrics.other.insert(key.into(), n);
    }
  }
  Some(post)
}
