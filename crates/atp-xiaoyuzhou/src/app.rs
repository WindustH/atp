//! Xiaoyuzhou's app API (`api.xiaoyuzhoufm.com`), the route of what only
//! a logged-in account sees: transcripts, the play history and progress,
//! a show's whole episode list. Requests go out as the iOS app sends
//! them: its ordinary identification headers (`BundleID`, `App-Version`,
//! `Model` …, no secret key), a device id of this install
//! (`x-jike-device-id`, random, kept with the session) and the access
//! token in `x-jike-access-token`.
//!
//! Xiaoyuzhou is made by Jike's company and renews tokens as Jike does:
//! when a call answers 401, the refresh token is traded through
//! `POST /app_auth_tokens.refresh` for a new pair, which is saved at once
//! (the old refresh token stops working), and the call is repeated once.

use atp_core::http::{Method, Req, Resp, status_error};
use atp_core::oauth::Tokens;
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt};
use rand::Rng;

pub const APP_API: &str = "https://api.xiaoyuzhoufm.com";
/// The app release whose headers ATP sends.
const APP_VERSION: &str = "2.98.0";
const APP_BUILD: &str = "2908";
const OS_VERSION: &str = "26.2.1";
const USER_AGENT: &str = "Xiaoyuzhou/2.98.0 (build:2908; iOS 26.2.1)";
/// Session extra: this install's device id.
const DEVICE_ID: &str = "device_id";
pub const LOGIN_HINT: &str = "log in: `atp xiaoyuzhou login-sms` (a texted code) or \
   `atp xiaoyuzhou login --token 'ACCESS_TOKEN|REFRESH_TOKEN'`";

/// The app API for one command.
pub struct App<'a> {
  pub ctx: &'a Ctx,
}

impl App<'_> {
  pub fn logged_in(&self) -> bool {
    Tokens::saved(self.ctx).is_some()
  }

  pub fn require_login(&self) -> Result<()> {
    if self.logged_in() {
      return Ok(());
    }
    Err(Error::auth("not logged in to Xiaoyuzhou's app API").with_hint(LOGIN_HINT))
  }

  /// `GET <APP_API><path>?<params>`, logged in.
  pub async fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    self.call(Method::GET, path, params, None).await
  }

  /// `POST <APP_API><path>` with a JSON body, logged in.
  pub async fn post(&self, path: &str, body: &Value) -> Result<Value> {
    self.call(Method::POST, path, &[], Some(body)).await
  }

  async fn call(
    &self,
    method: Method,
    path: &str,
    params: &[(&str, String)],
    body: Option<&Value>,
  ) -> Result<Value> {
    self.require_login()?;
    let token = Tokens::saved(self.ctx).map(|t| t.access);
    let send = |token: Option<String>| {
      let mut req = self
        .request(method.clone(), path, token.as_deref())
        .queries(params.iter().map(|(k, v)| (*k, v)));
      if let Some(body) = body {
        req = req.json(body);
      }
      req.send()
    };
    let resp = send(token).await?;
    if resp.status.as_u16() != 401 {
      return check(resp);
    }
    let fresh = self.renew().await?;
    check(send(Some(fresh)).await?)
  }

  /// A request with the app's headers, and the access token when given.
  pub fn request(&self, method: Method, path: &str, token: Option<&str>) -> Req<'_> {
    let now = jiff::Zoned::now();
    let mut req = self
      .ctx
      .http
      .request(method, format!("{APP_API}{path}"))
      .header("user-agent", USER_AGENT)
      .header("accept", "*/*")
      .header("accept-language", "zh-Hans-CN;q=1.0")
      .header("market", "AppStore")
      .header("app-buildno", APP_BUILD)
      .header("app-version", APP_VERSION)
      .header("os", "ios")
      .header("os-version", OS_VERSION)
      .header("manufacturer", "Apple")
      .header("model", "iPhone18,1")
      .header("bundleid", "app.podcast.cosmos")
      .header("abtest-info", r#"{"old_user_discovery_feed":"enable"}"#)
      .header("app-permissions", "100000")
      .header("wificonnected", "true")
      .header("x-custom-xiaoyuzhou-app-dev", "")
      .header("x-jike-device-id", self.device_id())
      .header("x-jike-device-properties", "")
      .header("local-time", now.timestamp().to_string())
      .header(
        "timezone",
        now.time_zone().iana_name().unwrap_or("Asia/Shanghai"),
      );
    if let Some(token) = token {
      req = req.header("x-jike-access-token", token);
    }
    req
  }

  /// This install's device id: a random UUID made once and kept with the
  /// session, as the app keeps its own.
  fn device_id(&self) -> String {
    if let Some(id) = self.ctx.extra(DEVICE_ID).filter(|id| !id.is_empty()) {
      return id;
    }
    let bytes: [u8; 16] = rand::rng().random();
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    let id = format!(
      "{}-{}-4{}-A{}-{}",
      &hex[..8],
      &hex[8..12],
      &hex[13..16],
      &hex[17..20],
      &hex[20..32]
    );
    self.ctx.set_extra(DEVICE_ID, &id);
    id
  }

  /// Keep a token pair and this install's device id in the session file
  /// at once (a renewed refresh token replaces the old one, which stops
  /// working).
  pub fn save(&self, tokens: &Tokens) -> Result<()> {
    tokens.save(self.ctx)?;
    let device = self.device_id();
    self.ctx.store.update_session(|session| {
      session.extra.insert(DEVICE_ID.to_owned(), device);
    })
  }

  /// A fresh access token from the saved refresh token, saved at once.
  async fn renew(&self) -> Result<String> {
    let saved = Tokens::saved(self.ctx).and_then(|t| t.refresh);
    let refresh_token =
      saved.ok_or_else(|| Error::auth("the Xiaoyuzhou session ended").with_hint(LOGIN_HINT))?;
    let renewed = refresh(self, &refresh_token).await?;
    self.save(&renewed)?;
    Ok(renewed.access)
  }
}

/// A fresh access token and the refresh token that replaces `refresh_token`.
pub async fn refresh(app: &App<'_>, refresh_token: &str) -> Result<Tokens> {
  let resp = app
    .request(Method::POST, "/app_auth_tokens.refresh", None)
    .header("x-jike-refresh-token", refresh_token)
    .bytes(
      Vec::new(),
      "application/x-www-form-urlencoded; charset=utf-8",
    )
    .send()
    .await?;
  if matches!(resp.status.as_u16(), 401 | 403) {
    return Err(Error::auth("Xiaoyuzhou refused the refresh token").with_hint(LOGIN_HINT));
  }
  token_answer(resp)?
    .ok_or_else(|| Error::auth("Xiaoyuzhou renewed no access token").with_hint(LOGIN_HINT))
}

/// The token pair an answer carries, in its headers (logins) or its body
/// (refreshes); the answer's error when it is one.
pub fn token_answer(resp: Resp) -> Result<Option<Tokens>> {
  let header = |name: &str| {
    resp
      .header(name)
      .filter(|t| !t.is_empty())
      .map(str::to_owned)
  };
  let (access, refresh) = (
    header("x-jike-access-token"),
    header("x-jike-refresh-token"),
  );
  let v = check(resp)?;
  let body = |name: &str| v.str(name).or_else(|| v.str(&format!("data.{name}")));
  Ok(
    access
      .or_else(|| body("x-jike-access-token"))
      .map(|access| Tokens {
        access,
        refresh: refresh.or_else(|| body("x-jike-refresh-token")),
        expires_at: None,
      }),
  )
}

/// The `loadMoreKey` a cursor stands for: the key as JSON text, or a
/// cursor typed by hand.
pub fn load_more_key(cursor: &str) -> Value {
  serde_json::from_str(cursor).unwrap_or_else(|_| Value::String(cursor.to_owned()))
}

/// The cursor of a listing's `loadMoreKey` (its JSON text), when it has
/// one and the page was not empty.
pub fn next_cursor(key: &Value, got: usize) -> Option<String> {
  let more = got > 0 && !key.is_null() && key.as_str() != Some("");
  more.then(|| key.to_string())
}

/// The JSON answer, or the error it stands for. Failures come as HTTP
/// errors with `{code, msg | message | toast}`, or as `success: false`.
pub fn check(resp: Resp) -> Result<Value> {
  let status = resp.status.as_u16();
  let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
    if status == 401 || status == 403 {
      return Err(Error::auth("Xiaoyuzhou did not accept the session").with_hint(LOGIN_HINT));
    }
    if !resp.status.is_success() {
      return Err(status_error(resp.status, &resp.text()));
    }
    return Err(Error::upstream(format!(
      "Xiaoyuzhou's app API answered with a page instead of data: {}",
      truncate(&one_line(&resp.text()), 160)
    )));
  };
  let code = v.i64("code").filter(|c| *c != 0 && *c != 200);
  if resp.status.is_success() && code.is_none() && v.bool("success") != Some(false) {
    return Ok(v);
  }
  let message = v
    .first_str(&["msg", "message", "toast", "error"])
    .unwrap_or_else(|| format!("HTTP {status}"));
  let text = format!("Xiaoyuzhou: {message}");
  Err(match (status, code) {
    (401 | 403, _) | (_, Some(401 | 403)) => Error::auth(text).with_hint(LOGIN_HINT),
    (429, _) => Error::new(ErrorCode::RateLimited, text).with_hint("wait a while before retrying"),
    (404, _) | (_, Some(404)) => Error::not_found(text),
    (400, _) => Error::input(text),
    _ => Error::upstream(text),
  })
}
