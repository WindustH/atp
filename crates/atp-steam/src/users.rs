//! Profiles and owned games through the Steam Web API, with a key the
//! person made (`login --token 'API_KEY|PROFILE'`). What a profile shows
//! follows its privacy settings: private profiles give their name and
//! picture only, and no games.

use std::cmp::Reverse;

use atp_core::{Error, ErrorCode, Post, Result, User, ValueExt, credentials};

use crate::api::{Api, KEY, PROFILE};
use crate::parse;
use crate::refs::{self, UserRef};

/// `games --sort`.
pub const GAME_SORTS: &[&str] = &["playtime", "recent", "name"];

/// `API_KEY|PROFILE`: the key, and your profile for `whoami` and `games`.
pub async fn login(api: &Api, token: &str) -> Result<()> {
  let fields = credentials::expect(token, 2, 2, "API_KEY|PROFILE")?;
  api.ctx.set_extra(KEY, &fields[0]);
  let id = steam_id(api, &fields[1]).await?;
  api.ctx.set_extra(PROFILE, &id);
  Ok(())
}

/// A profile argument → its SteamID64 (custom names are looked up).
pub async fn steam_id(api: &Api, arg: &str) -> Result<String> {
  let name = match refs::user(arg)? {
    UserRef::Id(id) => return Ok(id),
    UserRef::Vanity(name) => name,
  };
  let v = api
    .web(
      "ISteamUser/ResolveVanityURL/v1",
      vec![("vanityurl", name.clone())],
    )
    .await?;
  match (v.i64("response.success"), v.str("response.steamid")) {
    (Some(1), Some(id)) => Ok(id),
    _ => Err(Error::not_found(format!("no Steam profile named {name}"))),
  }
}

/// Your SteamID64, as saved at login.
fn mine(api: &Api) -> Result<String> {
  api.require()?;
  api.profile().ok_or_else(|| {
    Error::auth("no Steam profile is saved with the key")
      .with_hint("run `atp steam login --token 'API_KEY|PROFILE'` with your profile")
  })
}

pub async fn whoami(api: &Api) -> Result<User> {
  let id = mine(api)?;
  summary(api, &id).await
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let id = steam_id(api, arg).await?;
  summary(api, &id).await
}

async fn summary(api: &Api, id: &str) -> Result<User> {
  let v = api
    .web(
      "ISteamUser/GetPlayerSummaries/v2",
      vec![("steamids", id.to_owned())],
    )
    .await?;
  v.list("response.players")
    .first()
    .map(parse::player)
    .ok_or_else(|| Error::not_found(format!("no Steam profile {id}")))
}

/// The games a profile owns (or played in the last two weeks), in the
/// order of `sort`.
pub async fn games(api: &Api, user: Option<&str>, recent: bool, sort: &str) -> Result<Vec<Post>> {
  let id = match user {
    Some(user) => steam_id(api, user).await?,
    None => mine(api)?,
  };
  let (path, mut params) = if recent {
    ("IPlayerService/GetRecentlyPlayedGames/v1", vec![])
  } else {
    (
      "IPlayerService/GetOwnedGames/v1",
      vec![
        ("include_appinfo", "1".to_owned()),
        ("include_played_free_games", "1".to_owned()),
      ],
    )
  };
  params.push(("steamid", id.clone()));
  let v = api.web(path, params).await?;
  let response = v.at("response");
  if response.as_object().is_none_or(|o| o.is_empty()) {
    return Err(Error::new(
      ErrorCode::PermissionDenied,
      format!("Steam profile {id} keeps its game details private"),
    ));
  }
  let mut games: Vec<Post> = response.list("games").iter().map(parse::owned).collect();
  let raw = |post: &Post, key: &str| post.raw.as_ref().and_then(|v| v.u64(key));
  match sort {
    "name" => games.sort_by_key(|g| g.title.clone().unwrap_or_default().to_lowercase()),
    "recent" => games.sort_by_key(|g| Reverse(raw(g, "rtime_last_played"))),
    _ => games.sort_by_key(|g| Reverse(raw(g, "playtime_forever"))),
  }
  Ok(games)
}
