//! Logins, as nowcoder.com does them.
//!
//! - WeChat QR code: `GET /api/sparta/wechat/login/qr-code?source=0` hands
//!   out a `ticket` and the picture of a QR code of the site's WeChat
//!   official account (`imageUrl`, valid `expireSecond`, 120 s); scanning
//!   it follows 牛客 in WeChat and signs in. `POST /api/sparta/wechat/
//!   login/qr-code/check-status?ticket=…&source=0` fails until then, every
//!   3 s as the page asks, and then answers the login `token`, which the
//!   page keeps as the `t` cookie.
//! - Token: `login --token` with the `t` cookie of a logged-in browser;
//!   `--browser` and `--cookie` import the same cookie.

use atp_core::http::Method;
use atp_core::{Error, QrStatus, QrTicket, Result, ValueExt};
use jiff::Timestamp;

use crate::api::{Api, Body, Route, TOKEN};

/// What the WeChat login is for: 0 nowcoder.com (1 is its résumé site).
const SOURCE: &str = "0";
/// The extra of a ticket holding when its QR code expires (Unix seconds).
const EXPIRES: &str = "expires";

pub async fn qr_start(api: &Api) -> Result<QrTicket> {
  let params = [("source", SOURCE.to_owned())];
  let v = api
    .get(Route::Gateway, "/api/sparta/wechat/login/qr-code", &params)
    .await?;
  let (Some(ticket), Some(link)) = (v.str("ticket"), v.str("imageUrl")) else {
    return Err(Error::upstream("Nowcoder's WeChat login gave no QR code"));
  };
  let image = api
    .ctx
    .http
    .get(&link)
    .no_cookies()
    .send()
    .await?
    .check()?
    .body;
  if atp_core::file::sniff(&image).is_none() {
    return Err(Error::upstream(
      "the login QR code did not come as a picture",
    ));
  }
  let expires = Timestamp::now().as_second() + v.i64("expireSecond").unwrap_or(120);
  let mut ticket = QrTicket {
    url: link,
    token: ticket,
    image: Some(image),
    ..QrTicket::default()
  };
  ticket.extra.insert(EXPIRES.into(), expires.to_string());
  Ok(ticket)
}

/// The login once WeChat signed in; the check fails until then.
pub async fn qr_poll(api: &Api, ticket: &QrTicket) -> Result<QrStatus> {
  let expires = ticket
    .extra
    .get(EXPIRES)
    .and_then(|e| e.parse::<i64>().ok());
  if expires.is_some_and(|at| Timestamp::now().as_second() > at) {
    return Ok(QrStatus::Expired);
  }
  let params = [
    ("ticket", ticket.token.clone()),
    ("source", SOURCE.to_owned()),
  ];
  let checked = api
    .call(
      Method::POST,
      Route::Gateway,
      "/api/sparta/wechat/login/qr-code/check-status",
      &params,
      Body::None,
    )
    .await;
  let Ok(v) = checked else {
    return Ok(QrStatus::Waiting);
  };
  match v.str("token") {
    Some(token) => {
      api.ctx.http.set_cookie(TOKEN, &token);
      Ok(QrStatus::Confirmed)
    }
    None => Ok(QrStatus::Waiting),
  }
}

/// `login --token`: the `t` cookie's value (a `t=` before it is dropped).
pub fn token(api: &Api, token: &str) -> Result<()> {
  let token = token.trim();
  let token = token.strip_prefix("t=").unwrap_or(token);
  if token.is_empty() || token.contains(char::is_whitespace) || token.contains(';') {
    return Err(Error::input(
      "expected `login --token TOKEN` (the value of nowcoder.com's `t` cookie)",
    ));
  }
  api.ctx.http.set_cookie(TOKEN, token);
  Ok(())
}
