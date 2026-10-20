//! WeRead's official Agent Gateway (`POST
//! https://i.weread.qq.com/api/agent/gateway`), the route of WeRead's own
//! agent skill: one endpoint, a personal API key (`wrk-…`) as a Bearer
//! token, and a JSON body naming the call (`api_name`, such as
//! `/store/search`) and the skill version, with the call's parameters
//! flat beside them. Nested parameters are silently dropped by the
//! gateway, so they never nest here.
//!
//! `errcode` `-2010` / `-2012` mean the key was refused. An answer with
//! `upgrade_info` asks for a newer skill version: ATP warns that its
//! `SKILL_VERSION` should be bumped, and goes on with the answer.

use atp_core::credentials;
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt};

pub const GATEWAY: &str = "https://i.weread.qq.com/api/agent/gateway";
/// The version of WeRead's agent skill whose calls ATP makes.
pub const SKILL_VERSION: &str = "1.0.4";
/// Session extra: the API key `login --token` saved.
pub const API_KEY: &str = "api_key";
/// The environment variable of the key; it wins over the saved one.
pub const KEY_ENV: &str = "WEREAD_API_KEY";
const KEY_HINT: &str = "make a new key and log in with it: `atp weread login --token wrk-…` \
   (or set WEREAD_API_KEY)";

/// The gateway for one command, with the key it calls with.
pub struct Gateway<'a> {
  ctx: &'a Ctx,
  key: String,
}

/// The API key of this run: `WEREAD_API_KEY`, else the saved one.
pub fn key(ctx: &Ctx) -> Option<String> {
  credentials::env(KEY_ENV).or_else(|| ctx.extra(API_KEY).filter(|k| !k.is_empty()))
}

/// Whether a `login --token` value looks like a gateway key.
pub fn check_key(token: &str) -> Result<String> {
  let key = token.trim();
  if key.starts_with("wrk-") && key.len() > 8 && !key.contains(char::is_whitespace) {
    return Ok(key.to_owned());
  }
  Err(Error::input(
    "a WeRead API key starts with `wrk-` (`atp weread login --token wrk-…`)",
  ))
}

impl<'a> Gateway<'a> {
  pub fn new(ctx: &'a Ctx) -> Option<Self> {
    key(ctx).map(|key| Self { ctx, key })
  }

  /// The key, for display: its first and last characters.
  pub fn masked_key(&self) -> String {
    credentials::mask(&self.key)
  }

  /// Call `api_name` with flat parameters; empty and null ones are left
  /// out, as the skill does.
  pub async fn call(&self, api_name: &str, params: &[(&str, Value)]) -> Result<Value> {
    let mut body = serde_json::Map::new();
    body.insert("api_name".into(), api_name.into());
    body.insert("skill_version".into(), SKILL_VERSION.into());
    for (name, value) in params {
      if !value.is_null() && value.as_str() != Some("") {
        body.insert((*name).to_owned(), value.clone());
      }
    }
    let resp = self
      .ctx
      .http
      .post(GATEWAY)
      .no_cookies()
      .header("authorization", format!("Bearer {}", self.key))
      .json(&body)
      .send()
      .await?;
    let parsed = serde_json::from_slice::<Value>(&resp.body);
    if matches!(resp.status.as_u16(), 401 | 403) {
      let said = parsed.ok().and_then(|v| v.first_str(&["errmsg", "errMsg"]));
      let status = format!("HTTP {}", resp.status.as_u16());
      return Err(refused(
        &said.map_or(status.clone(), |m| format!("{status}: {m}")),
      ));
    }
    let Ok(v) = parsed else {
      if !resp.status.is_success() {
        return Err(atp_core::http::status_error(resp.status, &resp.text()));
      }
      return Err(Error::upstream(format!(
        "WeRead's gateway answered {api_name} with a page instead of data: {}",
        truncate(&one_line(&resp.text()), 120)
      )));
    };
    upgrade_warning(&v);
    check(api_name, v)
  }
}

/// Say that WeRead asks for a newer skill version than ATP sends.
fn upgrade_warning(v: &Value) {
  let info = v.at("upgrade_info");
  if info.is_null() || info.as_bool() == Some(false) {
    return;
  }
  let wanted = info
    .first_str(&["required_version", "version"])
    .unwrap_or_else(|| "a newer one".into());
  let message = info.str("message").unwrap_or_default();
  tracing::warn!(
    "WeRead's gateway wants skill_version {wanted} (ATP sends {SKILL_VERSION}); \
     its SKILL_VERSION should be bumped. {message}"
  );
}

/// The gateway's `{errcode, errmsg}` as a shared error.
fn check(api_name: &str, v: Value) -> Result<Value> {
  let code = v
    .first_str(&["errcode", "errCode"])
    .and_then(|c| c.parse::<i64>().ok())
    .unwrap_or(0);
  if code == 0 {
    return Ok(v);
  }
  let message = v.first_str(&["errmsg", "errMsg"]).unwrap_or_default();
  let text = format!("WeRead's gateway: {api_name}: {message} ({code})");
  Err(match code {
    -2010 | -2012 => refused(&text),
    -2003 | -2004 => Error::not_found(text),
    _ => Error::new(ErrorCode::UpstreamError, text),
  })
}

fn refused(detail: &str) -> Error {
  Error::auth(format!("WeRead refused the API key ({detail})")).with_hint(KEY_HINT)
}
