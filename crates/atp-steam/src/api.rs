//! Steam transport: the store's JSON API (`store.steampowered.com/api/…`
//! and `/appreviews/<app>`), open to everyone, and the Steam Web API
//! (`api.steampowered.com/<interface>/<method>/v<n>/`) with a key the
//! person makes, for public profiles and owned games.
//!
//! Prices and store texts follow a storefront country (`STEAM_CC`, `us`
//! by default) and a language (`STEAM_LANGUAGE`, `english` by default).
//! The key comes from `STEAM_API_KEY` or from the session (`login
//! --token`). Requests carry no cookies.

use atp_core::http::{Resp, status_error};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, credentials};

pub const STORE: &str = "https://store.steampowered.com";
pub const COMMUNITY: &str = "https://steamcommunity.com";
const WEB_API: &str = "https://api.steampowered.com";
pub const ENV_CC: &str = "STEAM_CC";
pub const ENV_LANGUAGE: &str = "STEAM_LANGUAGE";
pub const ENV_KEY: &str = "STEAM_API_KEY";
/// Session extras: the Web API key and the SteamID64 of your profile.
pub const KEY: &str = "api_key";
pub const PROFILE: &str = "steam_id";
const DEFAULT_CC: &str = "us";
const DEFAULT_LANGUAGE: &str = "english";
const LOGIN_HINT: &str =
  "run `atp steam login --token 'API_KEY|PROFILE'` (see `atp guide login steam`)";

/// Query parameters.
pub type Params = Vec<(&'static str, String)>;

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// The storefront country prices follow (`us`, `cn`, `de` …).
  pub fn cc(&self) -> Result<String> {
    match credentials::env(ENV_CC) {
      Some(cc) => country(&cc),
      None => Ok(DEFAULT_CC.to_owned()),
    }
  }

  /// The language of store texts (`english`, `schinese`, `japanese` …).
  pub fn language(&self) -> String {
    credentials::env(ENV_LANGUAGE)
      .map(|l| l.to_ascii_lowercase())
      .unwrap_or_else(|| DEFAULT_LANGUAGE.to_owned())
  }

  pub fn key(&self) -> Option<String> {
    credentials::env(ENV_KEY)
      .or_else(|| self.ctx.extra(KEY))
      .filter(|key| !key.is_empty())
  }

  pub fn require(&self) -> Result<String> {
    self
      .key()
      .ok_or_else(|| Error::auth("this needs a Steam Web API key").with_hint(LOGIN_HINT))
  }

  /// Your SteamID64, as saved at login.
  pub fn profile(&self) -> Option<String> {
    self.ctx.extra(PROFILE).filter(|id| !id.is_empty())
  }

  /// `GET <store>/<path>` (`api/appdetails`) as JSON.
  pub async fn store(&self, path: &str, params: Params) -> Result<Value> {
    let resp = self
      .ctx
      .http
      .get(format!("{STORE}/{path}"))
      .no_cookies()
      .header("accept", "application/json")
      .queries(params)
      .send()
      .await?;
    if !resp.status.is_success() {
      return Err(refused(&resp, "the Steam store"));
    }
    json(&resp, path)
  }

  /// `GET <web api>/<path>/` (`ISteamUser/GetPlayerSummaries/v2`) with the key.
  pub async fn web(&self, path: &str, mut params: Params) -> Result<Value> {
    params.push(("key", self.require()?));
    let resp = self
      .ctx
      .http
      .get(format!("{WEB_API}/{path}/"))
      .no_cookies()
      .queries(params)
      .send()
      .await?;
    match resp.status.as_u16() {
      401 | 403 => {
        Err(Error::auth("Steam refused the Web API key (HTTP 403)").with_hint(LOGIN_HINT))
      }
      _ if !resp.status.is_success() => Err(refused(&resp, "the Steam Web API")),
      _ => json(&resp, path),
    }
  }
}

/// A two-letter storefront country, lowercase.
pub fn country(cc: &str) -> Result<String> {
  let cc = cc.trim().to_ascii_lowercase();
  if cc.len() == 2 && cc.bytes().all(|b| b.is_ascii_lowercase()) {
    Ok(cc)
  } else {
    Err(Error::input(format!(
      "not a storefront country: {cc} (two letters, such as us, cn, de, jp)"
    )))
  }
}

fn json(resp: &Resp, path: &str) -> Result<Value> {
  serde_json::from_slice(&resp.body).map_err(|_| {
    Error::upstream(format!(
      "Steam answered {path} with something other than JSON"
    ))
  })
}

/// A failed answer: 429 (Steam limits bursts) and 404 by name.
fn refused(resp: &Resp, what: &str) -> Error {
  match resp.status.as_u16() {
    429 => Error::new(
      ErrorCode::RateLimited,
      format!("{what} asks for fewer requests (HTTP 429)"),
    )
    .with_hint("wait a minute or two and retry"),
    404 => Error::not_found(format!("{what} has nothing there (HTTP 404)")),
    _ => status_error(resp.status, &resp.text()),
  }
}
