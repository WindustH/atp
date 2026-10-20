//! Pinterest payloads → the shared model. Pins are posts with their
//! picture in original size (`images.orig`), the best MP4 of a video pin
//! and the pages of an idea pin as media; the link a pin leads to, its
//! board and the creator (when another account saved it) go in `extra`.
//! Users are users, boards collections whose id is `<username>/<board>`.

use atp_core::text::html_to_text;
use atp_core::{Collection, Media, Metrics, Post, User, Value, ValueExt, json};

use crate::api::SITE;

pub fn pin_url(id: &str) -> String {
  format!("{SITE}/pin/{id}/")
}

/// `Thu, 19 Oct 2023 10:00:00 +0000`.
fn time(v: &Value, path: &str) -> Option<jiff::Timestamp> {
  let text = v.str(path)?;
  jiff::fmt::strtime::parse("%a, %d %b %Y %H:%M:%S %z", text.trim())
    .ok()?
    .to_timestamp()
    .ok()
    .or_else(|| atp_core::text::parse_time(&text))
}

/// A text field, trimmed, or `None` when blank.
fn text(v: &Value, paths: &[&str]) -> Option<String> {
  paths
    .iter()
    .filter_map(|p| v.str(p))
    .map(|s| s.trim().to_owned())
    .find(|s| !s.is_empty())
}

/// An account of a list (the search of people), with its payload.
pub fn user(v: &Value) -> Option<User> {
  let mut user = account(v)?;
  user.raw = Some(v.clone());
  Some(user)
}

/// A profile (`UserResource`): an account with when it joined and its
/// monthly views.
pub fn profile(v: &Value) -> Option<User> {
  let mut user = user(v)?;
  user.created_at = time(v, "created_at");
  if let Some(views) = v.count("profile_reach") {
    user.stats.other.insert("monthly_views".into(), views);
  }
  Some(user)
}

/// The account of a pin or board, without its payload.
fn account(v: &Value) -> Option<User> {
  let name = v.str("username")?;
  let mut user = User {
    id: v.str("id").unwrap_or_else(|| name.clone()),
    name: text(v, &["full_name", "first_name"]).unwrap_or_else(|| name.clone()),
    url: Some(format!("{SITE}/{name}/")),
    avatar: v.first_str(&[
      "image_xlarge_url",
      "image_large_url",
      "image_medium_url",
      "image_small_url",
    ]),
    bio: text(v, &["about"]),
    verified: v
      .at("verified_identity")
      .as_object()
      .is_some_and(|o| !o.is_empty())
      || v.bool("is_verified_merchant") == Some(true),
    handle: Some(name),
    ..User::default()
  };
  user.stats.followers = v.count("follower_count");
  user.stats.following = v.count("following_count");
  user.stats.posts = v.count("pin_count");
  if let Some(boards) = v.count("board_count") {
    user.stats.other.insert("boards".into(), boards);
  }
  if let Some(site) = v.first_str(&["website_url", "domain_url"]) {
    user.extra.insert("website".into(), json!(site));
  }
  Some(user)
}

/// The biggest picture of an `images` map (`orig`, `736x` …).
fn picture(images: &Value) -> Option<Media> {
  let best = images.at("orig");
  let best = if best.str("url").is_some() {
    best
  } else {
    images
      .as_object()?
      .values()
      .filter(|i| i.str("url").is_some())
      .max_by_key(|i| i.u64("width").unwrap_or(0))?
  };
  let mut media = Media::image(best.str("url")?);
  media.width = best.u64("width").map(|w| w as u32);
  media.height = best.u64("height").map(|h| h as u32);
  Some(media)
}

/// The best MP4 of a `video_list` (HLS playlists are skipped).
fn video(list: &Value) -> Option<Media> {
  let best = list
    .as_object()?
    .values()
    .filter(|v| v.str("url").is_some_and(|u| u.contains(".mp4")))
    .max_by_key(|v| v.u64("width").unwrap_or(0))?;
  let mut media = Media::video(best.str("url")?);
  media.width = best.u64("width").map(|w| w as u32);
  media.height = best.u64("height").map(|h| h as u32);
  media.duration = best.f64("duration").map(|ms| ms / 1000.0);
  Some(media)
}

/// The pictures and videos of a pin: its video, the pages of an idea pin,
/// else its picture.
fn media(v: &Value) -> Vec<Media> {
  if let Some(clip) = video(v.at("videos.video_list")) {
    return vec![clip];
  }
  let pages: Vec<Media> = v
    .list("story_pin_data.pages")
    .iter()
    .flat_map(|page| page.list("blocks"))
    .filter_map(|block| {
      video(block.at("video.video_list")).or_else(|| picture(block.at("image.images")))
    })
    .collect();
  if !pages.is_empty() {
    return pages;
  }
  let alt = text(v, &["alt_text", "auto_alt_text"]);
  picture(v.at("images"))
    .map(|mut m| {
      m.alt = alt;
      m
    })
    .into_iter()
    .collect()
}

pub fn pin(v: &Value) -> Post {
  let id = v.str("id").unwrap_or_default();
  let reactions: u64 = v
    .at("reaction_counts")
    .as_object()
    .map(|o| o.values().filter_map(|n| n.as_u64()).sum())
    .unwrap_or(0);
  let mut post = Post {
    kind: "pin".into(),
    title: text(v, &["title", "grid_title", "seo_title"]),
    text: text(
      v,
      &[
        "description",
        "closeup_unified_description",
        "closeup_description",
      ],
    )
    .map(|t| html_to_text(&t)),
    url: Some(pin_url(&id)),
    author: account(v.at("pinner")),
    created_at: time(v, "created_at"),
    metrics: Metrics {
      favorites: v.first_count(&["aggregated_pin_data.aggregated_stats.saves", "repin_count"]),
      comments: v.first_count(&["aggregated_pin_data.comment_count", "comment_count"]),
      likes: (reactions > 0).then_some(reactions),
      ..Metrics::default()
    },
    media: media(v),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  details(&mut post, v);
  post
}

/// The `extra` keys of a pin: where it leads, its board, its creator when
/// someone else saved it, its main color and the ad mark.
fn details(post: &mut Post, v: &Value) {
  let creator = v.str("native_creator.username");
  let fields = [
    ("link", text(v, &["link"]).map(|l| json!(l))),
    (
      "domain",
      text(v, &["domain"])
        .filter(|_| v.str("link").is_some())
        .map(|d| json!(d)),
    ),
    (
      "board",
      v.at("board").str("name").map(|name| {
        json!({
          "id": v.str("board.id"),
          "name": name,
          "url": v.str("board.url").map(|u| format!("{SITE}{u}")),
        })
      }),
    ),
    (
      "creator",
      creator
        .filter(|c| Some(c) != v.str("pinner.username").as_ref())
        .map(|c| json!(c)),
    ),
    ("dominant_color", v.str("dominant_color").map(|c| json!(c))),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      post.extra.insert(key.into(), value);
    }
  }
  if v.bool("is_promoted") == Some(true) {
    post.extra.insert("promoted".into(), json!(true));
  }
}

/// A board's id: `<username>/<board>` from its path (`/user/board/`).
pub fn board_path(v: &Value) -> Option<String> {
  let url = v.str("url")?;
  let path = atp_core::text::decode_uri_component(url.trim_matches('/'));
  path.contains('/').then_some(path)
}

pub fn board(v: &Value) -> Collection {
  let mut board = Collection {
    id: board_path(v).or_else(|| v.str("id")).unwrap_or_default(),
    kind: "board".into(),
    name: v.str("name").unwrap_or_default(),
    description: text(v, &["description"]),
    url: v.str("url").map(|u| format!("{SITE}{u}")),
    items: v.count("pin_count"),
    followers: v.count("follower_count"),
    owner: account(v.at("owner")),
    raw: Some(v.clone()),
    ..Collection::default()
  };
  if let Some(id) = v.str("id") {
    board.extra.insert("board_id".into(), json!(id));
  }
  if let Some(privacy) = v.str("privacy").filter(|p| p != "public") {
    board.extra.insert("privacy".into(), json!(privacy));
  }
  if let Some(sections) = v.count("section_count").filter(|n| *n > 0) {
    board.extra.insert("sections".into(), json!(sections));
  }
  board
}
