//! Steam Web API profiles (users) and the games they own (posts).

use atp_core::text::from_secs;
use atp_core::{Media, Post, User, Value, ValueExt, json};

use super::{app_url, hours, profile_url};

/// What `personastate` means.
const STATES: &[&str] = &[
  "offline",
  "online",
  "busy",
  "away",
  "snooze",
  "looking to trade",
  "looking to play",
];

/// A Web API player summary.
pub fn player(v: &Value) -> User {
  let id = v.str("steamid").unwrap_or_default();
  let url = v.str("profileurl").unwrap_or_else(|| profile_url(&id));
  let handle = url
    .split("/id/")
    .nth(1)
    .map(|name| name.trim_end_matches('/').to_owned());
  let location: Vec<String> = [v.str("locstatecode"), v.str("loccountrycode")]
    .into_iter()
    .flatten()
    .collect();
  let mut user = User {
    name: v.str("personaname").unwrap_or_else(|| id.clone()),
    handle,
    url: Some(url),
    avatar: v.first_str(&["avatarfull", "avatarmedium", "avatar"]),
    location: Some(location.join(", ")).filter(|l| !l.is_empty()),
    created_at: v.i64("timecreated").and_then(from_secs),
    raw: Some(v.clone()),
    id,
    ..User::default()
  };
  let state = v.u64("personastate").and_then(|s| STATES.get(s as usize));
  let public = v.u64("communityvisibilitystate") == Some(3);
  let fields = [
    ("state", state.map(|s| json!(s))),
    ("public", Some(json!(public))),
    ("playing", v.str("gameextrainfo").map(Value::from)),
    ("real_name", v.str("realname").map(Value::from)),
    (
      "last_online",
      v.i64("lastlogoff")
        .and_then(from_secs)
        .map(|t| json!(t.to_string())),
    ),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      user.extra.insert(key.into(), value);
    }
  }
  user
}

/// A game of `GetOwnedGames` / `GetRecentlyPlayedGames`, with play time.
pub fn owned(v: &Value) -> Post {
  let id = v.str("appid").unwrap_or_default();
  let icon = v.str("img_icon_url").map(|hash| {
    format!("https://media.steampowered.com/steamcommunity/public/images/apps/{id}/{hash}.jpg")
  });
  let mut post = Post {
    kind: "game".into(),
    title: v.str("name"),
    url: Some(app_url(&id)),
    media: icon.map(Media::image).into_iter().collect(),
    raw: Some(v.clone()),
    id,
    ..Post::default()
  };
  let fields = [
    ("hours", hours(v.u64("playtime_forever"))),
    ("hours_two_weeks", hours(v.u64("playtime_2weeks"))),
    (
      "last_played",
      v.i64("rtime_last_played")
        .and_then(from_secs)
        .map(|t| json!(t.to_string())),
    ),
  ];
  for (key, value) in fields {
    if let Some(value) = value {
      post.extra.insert(key.into(), value);
    }
  }
  post
}
