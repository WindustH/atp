//! The login panel's WeChat login, a mini-program code:
//!
//! - `GET apigateway…/api/account/v3.0/mina/create/minacode/v2?scene=…`
//!   answers the picture of a code of Shixiseng's WeChat mini program
//!   (base64 JPEG) for a `scene` of 16 hex digits the page makes up;
//! - scanning it in WeChat opens the mini program, which asks to confirm;
//! - `POST …/api/account/v2.0/mina/polling?scene=…` waits about 20 s and
//!   answers `{code: 500, msg: {isLogin: false, isScan}}` until then,
//!   `{code: 100, msg: {isLogin: true}}` once confirmed, with the session
//!   cookie. The page gives up after six waits, about two minutes.

use atp_core::http::Method;
use atp_core::{Error, ErrorCode, QrStatus, QrTicket, Result, ValueExt};
use base64::Engine;
use jiff::Timestamp;

use crate::api::{Api, SESSION, WWW};

const CODE: &str = "/api/account/v3.0/mina/create/minacode/v2";
const POLLING: &str = "/api/account/v2.0/mina/polling";
/// The mini program's page the code opens.
const PAGE: &str = "pages/login/pcloginpage/pcloginpage";
/// How long a code is waited for, as the page does.
const LIFETIME: i64 = 120;
/// The extra of a ticket holding when its code expires (Unix seconds).
const EXPIRES: &str = "expires";

/// 16 hex digits as the page makes them: `xxxxxxx4xxyxxxxx`, `y` one of
/// 8, 9, a, b.
fn scene() -> String {
  let bytes: [u8; 8] = rand::random();
  let mut hex: Vec<char> = bytes
    .iter()
    .flat_map(|b| format!("{b:02x}").chars().collect::<Vec<_>>())
    .collect();
  hex[7] = '4';
  hex[10] = ['8', '9', 'a', 'b'][usize::from(bytes[5] & 3)];
  hex.into_iter().collect()
}

pub async fn qr_start(api: &Api) -> Result<QrTicket> {
  let scene = scene();
  let params = [
    ("scene", scene.clone()),
    ("pages", PAGE.to_owned()),
    ("stype", "common".to_owned()),
    ("utm_source", String::new()),
    ("utm_campaign", String::new()),
  ];
  let v = api.gateway(Method::GET, CODE, &params).await?;
  let picture = v
    .str("msg")
    .filter(|_| v.i64("code") == Some(100))
    .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
    .filter(|bytes| atp_core::file::sniff(bytes).is_some());
  let Some(image) = picture else {
    return Err(Error::upstream(format!(
      "Shixiseng gave no login code: {}",
      v.str("msg").unwrap_or_else(|| v.to_string())
    )));
  };
  let mut ticket = QrTicket {
    url: format!("{WWW}/user/login"),
    token: scene,
    image: Some(image),
    ..QrTicket::default()
  };
  let expires = Timestamp::now().as_second() + LIFETIME;
  ticket.extra.insert(EXPIRES.into(), expires.to_string());
  Ok(ticket)
}

/// One wait of the polling; the session cookie once confirmed.
pub async fn qr_poll(api: &Api, ticket: &QrTicket) -> Result<QrStatus> {
  let expires = ticket
    .extra
    .get(EXPIRES)
    .and_then(|e| e.parse::<i64>().ok());
  if expires.is_some_and(|at| Timestamp::now().as_second() > at) {
    return Ok(QrStatus::Expired);
  }
  let params = [("scene", ticket.token.clone())];
  let v = match api.gateway(Method::POST, POLLING, &params).await {
    Ok(v) => v,
    Err(e) if e.code == ErrorCode::VerificationRequired => return Err(e),
    Err(_) => return Ok(QrStatus::Waiting),
  };
  if v.i64("code") == Some(100) && v.bool("msg.isLogin") == Some(true) {
    if !api.ctx.http.has_cookie(SESSION) {
      return Err(
        Error::auth("Shixiseng confirmed the login but handed out no session cookie")
          .with_hint("log in with `atp shixiseng login --browser` instead"),
      );
    }
    return Ok(QrStatus::Confirmed);
  }
  if v.bool("msg.isScan") == Some(true) {
    return Ok(QrStatus::Scanned);
  }
  Ok(QrStatus::Waiting)
}
