//! Apps and profiles as arguments.
//!
//! - apps: the id (`620`), `app/620`, or a store or community link
//!   (`https://store.steampowered.com/app/620/Portal_2/`,
//!   `https://steamcommunity.com/app/620`);
//! - profiles: a SteamID64 (`76561197960287930`), a
//!   `steamcommunity.com/profiles/<id>` or `/id/<name>` link, `@name` or
//!   the custom name.

use atp_core::{Error, Result};

/// A profile: its SteamID64, or the custom name of its `/id/` link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRef {
  Id(String),
  Vanity(String),
}

/// The app id an argument names.
pub fn app(arg: &str) -> Result<u64> {
  let arg = arg.trim();
  let id = match arg.find("app/") {
    Some(at) => arg[at + 4..].split(['/', '?', '#']).next().unwrap_or(""),
    None => arg,
  };
  id.parse().ok().filter(|id| *id > 0).ok_or_else(|| {
    Error::input(format!(
      "not a Steam app: {arg} (expected an app id or a store link)"
    ))
  })
}

/// The profile an argument names.
pub fn user(arg: &str) -> Result<UserRef> {
  let arg = arg.trim().trim_end_matches('/');
  let segment = |marker: &str| {
    arg
      .find(marker)
      .and_then(|at| arg[at + marker.len()..].split(['/', '?', '#']).next())
      .map(str::to_owned)
  };
  if let Some(id) = segment("profiles/").filter(|id| steam_id(id)) {
    return Ok(UserRef::Id(id));
  }
  if let Some(name) = segment("id/").filter(|name| vanity(name)) {
    return Ok(UserRef::Vanity(name));
  }
  if steam_id(arg) {
    return Ok(UserRef::Id(arg.to_owned()));
  }
  let name = arg.trim_start_matches('@');
  if vanity(name) && !arg.contains('/') {
    return Ok(UserRef::Vanity(name.to_owned()));
  }
  Err(Error::input(format!(
    "not a Steam profile: {arg} (expected a SteamID64, a profile link or a custom name)"
  )))
}

/// A SteamID64 of an individual account: 17 digits from 7656119….
fn steam_id(s: &str) -> bool {
  s.len() == 17 && s.starts_with("7656119") && s.bytes().all(|b| b.is_ascii_digit())
}

fn vanity(s: &str) -> bool {
  (2..=32).contains(&s.len())
    && s
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
