//! Logins, as eleduck.com does them.
//!
//! - WeChat QR code: `POST /auth/wechat/token` hands out a `wechat_token`;
//!   the QR code holds `https://eleduck.com/auth/wechat/manual?
//!   wechat_token=…&kind=signin`, which WeChat opens when it scans it and
//!   which signs the WeChat account in; `GET /auth/wechat/token?
//!   wechat_token=` answers the login `token` once that is done.
//! - Token: the `token` cookie of a logged-in browser, given to `login
//!   --token`. `--browser` and `--cookie` import the same cookie.
//!
//! Either way the token is kept as the session's `token` cookie, which
//! [`Api`] sends as `Authorization: Bearer`.

use atp_core::http::Method;
use atp_core::text::encode_uri_component;
use atp_core::{Error, ErrorCode, QrStatus, QrTicket, Result, Value, ValueExt};

use crate::api::{Api, TOKEN, WEB};

pub async fn qr_start(api: &Api) -> Result<QrTicket> {
  let v = api
    .anonymous(Method::POST, "/auth/wechat/token", &[])
    .await?
    .check()?
    .value()?;
  let token = v
    .str("wechat_token")
    .ok_or_else(|| Error::upstream("Eleduck's WeChat login gave no token"))?;
  Ok(QrTicket {
    url: format!(
      "{WEB}/auth/wechat/manual?wechat_token={}&kind=signin",
      encode_uri_component(&token)
    ),
    token,
    ..QrTicket::default()
  })
}

/// The login token once WeChat signed in; a refused ticket has expired,
/// as the site treats it.
pub async fn qr_poll(api: &Api, ticket: &QrTicket) -> Result<QrStatus> {
  let params = [("wechat_token", ticket.token.clone())];
  let sent = api
    .anonymous(Method::GET, "/auth/wechat/token", &params)
    .await;
  let resp = match sent {
    Ok(resp) => resp,
    Err(e) if e.code == ErrorCode::NetworkError => return Ok(QrStatus::Waiting),
    Err(e) => return Err(e),
  };
  if !resp.status.is_success() {
    return Ok(QrStatus::Expired);
  }
  let v: Value = serde_json::from_slice(&resp.body).unwrap_or_default();
  match v.str("token") {
    Some(token) => {
      api.ctx.http.set_cookie(TOKEN, &token);
      Ok(QrStatus::Confirmed)
    }
    None => Ok(QrStatus::Waiting),
  }
}

/// `login --token`: the `token` cookie's value (a `token=` before it is
/// dropped).
pub fn token(api: &Api, token: &str) -> Result<()> {
  let token = token.trim();
  let token = token.strip_prefix("token=").unwrap_or(token);
  if token.is_empty() || token.contains(char::is_whitespace) || token.contains(';') {
    return Err(Error::input(
      "expected `login --token TOKEN` (the value of eleduck.com's `token` cookie)",
    ));
  }
  api.ctx.http.set_cookie(TOKEN, token);
  Ok(())
}
