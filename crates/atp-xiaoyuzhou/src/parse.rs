//! Page data and app answers → posts, comments, users and transcripts. A
//! show is a post of kind `show` linking to its page; an episode is a
//! post of kind `episode` whose author is its show and whose media is the
//! audio file the page plays (the publisher's file on Xiaoyuzhou's CDN);
//! hot comments keep the replies the page shows under them. The app's
//! answers carry the same objects as the pages.

use atp_core::text::html_to_text;
use atp_core::{Comment, Cue, Media, MediaKind, Metrics, Post, User, Value, ValueExt, json};
use jiff::Timestamp;

use crate::refs::{episode_url, show_url, user_url};

fn time(v: &Value, path: &str) -> Option<Timestamp> {
  v.str(path).and_then(|t| t.parse().ok())
}

fn put(extra: &mut atp_core::Extra, pairs: Vec<(&str, Value)>) {
  for (key, value) in pairs {
    if !value.is_null() {
      extra.insert(key.into(), value);
    }
  }
}

/// The largest picture of an image object.
fn picture(v: &Value, path: &str) -> Option<String> {
  let image = v.at(path);
  image.first_str(&["picUrl", "largePicUrl", "middlePicUrl", "smallPicUrl"])
}

/// A show as the author of its episodes.
fn show_as_author(show: &Value, pid: Option<String>) -> Option<User> {
  let pid = pid.or_else(|| show.str("pid"))?;
  Some(User {
    name: show.str("title").unwrap_or_else(|| pid.clone()),
    url: Some(show_url(&pid)),
    avatar: picture(show, "image"),
    id: pid,
    ..User::default()
  })
}

/// A user, podcaster or comment author (`uid`, `nickname`, `avatar`).
pub fn user(v: &Value) -> User {
  let id = v.str("uid").unwrap_or_default();
  let mut user = User {
    name: v.str("nickname").unwrap_or_else(|| id.clone()),
    url: (!id.is_empty()).then(|| user_url(&id)),
    avatar: picture(v, "avatar.picture"),
    bio: v.str("bio"),
    location: v.str("ipLoc"),
    raw: Some(v.clone()),
    id,
    ..User::default()
  };
  put(&mut user.extra, vec![("gender", json!(v.str("gender")))]);
  user
}

/// A show (`/podcast/<id>`, or a user's hosted show).
pub fn show(v: &Value) -> Post {
  let id = v.str("pid").unwrap_or_default();
  let mut post = Post {
    url: Some(show_url(&id)),
    kind: "show".into(),
    title: v.str("title"),
    text: v.first_str(&["description", "brief"]),
    author: v.str("author").map(|name| User {
      id: name.clone(),
      name,
      ..User::default()
    }),
    updated_at: time(v, "latestEpisodePubDate"),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  post.metrics.other.extend(
    v.u64("subscriptionCount")
      .map(|n| ("subscribers".to_owned(), n)),
  );
  let hosts: Vec<Value> = v
    .list("podcasters")
    .iter()
    .map(|p| json!({"id": p.str("uid"), "name": p.str("nickname"), "url": p.str("uid").map(|id| user_url(&id))}))
    .collect();
  put(
    &mut post.extra,
    vec![
      ("brief", json!(v.str("brief"))),
      ("episodes", json!(v.u64("episodeCount"))),
      ("image", json!(picture(v, "image"))),
      (
        "podcasters",
        if hosts.is_empty() {
          Value::Null
        } else {
          json!(hosts)
        },
      ),
      ("pay_type", json!(v.str("payType").filter(|t| t != "FREE"))),
    ],
  );
  post
}

/// The audio file of an episode, when the page gives one.
fn audio(v: &Value) -> Option<Media> {
  let url = v.first_str(&["enclosure.url", "media.source.url"])?;
  let mime = v.str("media.mimeType").unwrap_or_default();
  let kind = match MediaKind::from_mime(&mime) {
    MediaKind::Video => MediaKind::Video,
    _ => MediaKind::Audio,
  };
  let mut media = Media::new(kind, url);
  media.duration = v.f64("duration");
  media.size = v.u64("media.size");
  Some(media)
}

/// An episode (`/episode/<id>`, or one of a show's).
pub fn episode(v: &Value) -> Post {
  let id = v.str("eid").unwrap_or_default();
  let notes = v.str("shownotes").map(|html| html_to_text(&html));
  let mut post = Post {
    url: Some(episode_url(&id)),
    kind: "episode".into(),
    title: v.str("title"),
    text: notes.or_else(|| v.str("description")),
    author: show_as_author(v.at("podcast"), v.str("pid")),
    created_at: time(v, "pubDate"),
    metrics: Metrics {
      views: v.u64("playCount"),
      likes: v.u64("clapCount"),
      comments: v.u64("commentCount"),
      favorites: v.u64("favoriteCount"),
      ..Metrics::default()
    },
    media: audio(v).into_iter().collect(),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  put(
    &mut post.extra,
    vec![
      ("show_id", json!(v.str("pid"))),
      ("show", json!(v.str("podcast.title"))),
      ("duration", json!(v.u64("duration"))),
      (
        "image",
        json!(picture(v, "image").or_else(|| picture(v, "podcast.image"))),
      ),
      ("pay_type", json!(v.str("payType").filter(|t| t != "FREE"))),
    ],
  );
  post
}

/// A comment and the replies the page shows under it.
pub fn comment(v: &Value) -> Comment {
  let author = user(v.at("author"));
  Comment {
    id: v.str("id").unwrap_or_default(),
    text: v.str("text").unwrap_or_default(),
    created_at: time(v, "createdAt"),
    likes: v.u64("likeCount"),
    reply_count: v.u64("replyCount"),
    reply_to: v.str("replyToComment.author.nickname"),
    location: author.location.clone(),
    replies: v.list("replies").iter().map(comment).collect(),
    author: Some(author),
    raw: Some(v.clone()),
    ..Comment::default()
  }
}

/// A user page (`/user/<id>`): the profile, counts and hosted shows.
pub fn profile(v: &Value) -> User {
  let mut user = user(v.at("user"));
  let stats = v.at("userStats");
  user.stats.followers = stats.u64("followerCount");
  user.stats.following = stats.u64("followingCount");
  user.stats.other.extend(
    stats
      .u64("realSubscriptionCount")
      .map(|n| ("subscriptions".to_owned(), n)),
  );
  let shows: Vec<Value> = v
    .list("ownedPodcasts")
    .iter()
    .map(|s| json!({"id": s.str("pid"), "title": s.str("title"), "url": s.str("pid").map(|id| show_url(&id))}))
    .collect();
  put(
    &mut user.extra,
    vec![
      (
        "shows",
        if shows.is_empty() {
          Value::Null
        } else {
          json!(shows)
        },
      ),
      ("played_seconds", json!(stats.u64("totalPlayedSeconds"))),
    ],
  );
  user.raw = Some(v.clone());
  user
}

/// An episode of the play history, with how far it was played: seconds
/// (`extra.progress`), percent, when, and whether to the end.
pub fn played(v: &Value, progress: Option<&Value>) -> Post {
  let mut post = episode(v);
  let seconds = progress.and_then(|p| p.u64("progress"));
  let percent = seconds
    .zip(v.u64("duration").filter(|d| *d > 0))
    .map(|(s, d)| (s as f64 * 1000.0 / d as f64).round() / 10.0);
  put(
    &mut post.extra,
    vec![
      ("progress", json!(seconds)),
      ("progress_percent", json!(percent)),
      (
        "played_at",
        json!(
          progress
            .and_then(|p| time(p, "playedAt"))
            .map(|t| t.to_string())
        ),
      ),
      ("finished", json!(v.bool("isFinished"))),
    ],
  );
  post
}

/// Keys of a segment's start and end, in seconds or milliseconds.
const STARTS: &[&str] = &[
  "start",
  "startTime",
  "start_time",
  "begin",
  "beginTime",
  "bg",
];
const ENDS: &[&str] = &["end", "endTime", "end_time", "ed", "stop"];
const STARTS_MS: &[&str] = &["startMs", "start_ms", "beginMs"];
const ENDS_MS: &[&str] = &["endMs", "end_ms"];

/// One segment as read: times in milliseconds when the key says so,
/// else in a unit decided over the whole file.
struct Segment {
  start_ms: Option<f64>,
  end_ms: Option<f64>,
  start: Option<f64>,
  end: Option<f64>,
  text: String,
}

/// The timed lines of a transcript file: a list of segments (`text` with
/// a start and an end) or one text. `duration` (the episode's, seconds)
/// tells seconds from milliseconds when the keys do not.
pub fn cues(v: &Value, duration: Option<f64>) -> Vec<Cue> {
  let rows = match v {
    Value::Array(rows) => rows.as_slice(),
    _ => ["segments", "data", "transcript", "items", "sentences"]
      .iter()
      .map(|k| v.list(k))
      .find(|rows| !rows.is_empty())
      .unwrap_or(&[]),
  };
  if rows.is_empty() {
    let text = v.str("text").map(|t| t.trim().to_owned());
    return text
      .filter(|t| !t.is_empty())
      .map(|text| Cue {
        from: 0.0,
        to: duration.unwrap_or(0.0),
        text,
      })
      .into_iter()
      .collect();
  }
  let segments: Vec<Segment> = rows.iter().filter_map(segment).collect();
  let latest = segments
    .iter()
    .flat_map(|s| [s.start, s.end])
    .flatten()
    .fold(0.0, f64::max);
  let in_ms = match duration {
    Some(d) if d > 0.0 => latest > d * 2.0 + 60.0,
    _ => latest > 86_400.0,
  };
  let scale = if in_ms { 1000.0 } else { 1.0 };
  let times: Vec<(f64, Option<f64>)> = segments
    .iter()
    .map(|s| {
      let start = s
        .start_ms
        .map(|t| t / 1000.0)
        .or(s.start.map(|t| t / scale));
      let end = s.end_ms.map(|t| t / 1000.0).or(s.end.map(|t| t / scale));
      (start.unwrap_or(0.0), end)
    })
    .collect();
  segments
    .into_iter()
    .enumerate()
    .map(|(i, s)| {
      let (from, end) = times[i];
      let next = times.get(i + 1).map(|(start, _)| *start);
      Cue {
        from,
        to: end.or(next).unwrap_or(from).max(from),
        text: s.text,
      }
    })
    .collect()
}

fn segment(v: &Value) -> Option<Segment> {
  let text = v
    .first_str(&["text", "content", "sentence"])?
    .trim()
    .to_owned();
  if text.is_empty() {
    return None;
  }
  Some(Segment {
    start_ms: seconds(v, STARTS_MS),
    end_ms: seconds(v, ENDS_MS),
    start: seconds(v, STARTS),
    end: seconds(v, ENDS),
    text,
  })
}

/// The first of `keys` as a number: plain, or `01:02.5` / `1:02:03,500`.
fn seconds(v: &Value, keys: &[&str]) -> Option<f64> {
  keys.iter().find_map(|k| match v.at(k) {
    Value::Number(n) => n.as_f64(),
    Value::String(s) => s
      .trim()
      .replace(',', ".")
      .split(':')
      .try_fold(0.0, |total, part| {
        part.parse::<f64>().ok().map(|n| total * 60.0 + n)
      }),
    _ => None,
  })
}
