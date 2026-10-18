//! The QR login of the site's login page, scanned with the BOSS app:
//! `zppassport/captcha/randkey` hands out a `qrId`,
//! `zpweixin/qrcode/getqrcode?content=<qrId>` its picture;
//! `zppassport/qrcode/scan?uuid=<qrId>` answers `scaned` once the app
//! has scanned it and `zppassport/qrcode/scanLogin?qrId=<qrId>` `login`
//! once the login is confirmed on the phone (both wait up to half a
//! minute for news, as the page polls them); then
//! `zppassport/qrcode/dispatcher?qrId=<qrId>&pk=header-login` sets the
//! session's cookies (`wt2`, `wbg`, `zp_at`, `bst`). Plain requests,
//! like the login page's, without the security script's cookies.

use std::cell::Cell;
use std::time::Duration;

use atp_core::http::{Method, Req, Resp};
use atp_core::{Error, ErrorCode, QrStatus, QrTicket, Result, Value, ValueExt};

use crate::api::{Api, WWW, code};

/// How long a waiting call may take; the site answers within 30 s.
const LONG_POLL: Duration = Duration::from_secs(40);
/// The login page the requests come from.
const REFERER: &str = "https://www.zhipin.com/web/user/?ka=header-login";

/// A request of the login page, with the session's cookies but the
/// security script's ([`Api::cookies`]).
fn request<'a>(api: &'a Api, method: Method, path: &str, query: &[(&str, String)]) -> Req<'a> {
  let req = api
    .ctx
    .http
    .request(method, format!("{WWW}{path}"))
    .no_cookies()
    .header("referer", REFERER)
    .queries(query.iter().map(|(k, v)| (*k, v.as_str())));
  match api.cookies() {
    cookies if cookies.is_empty() => req,
    cookies => req.header("cookie", cookies),
  }
}

/// A call of the passport API, the whole answer. `wait` is how long a
/// waiting call may take.
async fn passport(
  api: &Api,
  method: Method,
  path: &str,
  query: &[(&str, String)],
  wait: Option<Duration>,
) -> Result<Value> {
  let mut req = request(api, method, path, query)
    .header("accept", "application/json, text/plain, */*")
    .header("x-requested-with", "XMLHttpRequest");
  if let Some(wait) = wait {
    req = req.timeout(wait).no_throttle();
  }
  let resp = req.send().await?;
  api.keep(&resp);
  resp.check()?.value()
}

/// A GET of the passport API whose answer is not JSON (the QR code's
/// picture, the page a login goes on to), its cookies kept; a redirect
/// is not followed.
async fn raw(api: &Api, path: &str, query: &[(&str, String)]) -> Result<Resp> {
  let resp = request(api, Method::GET, path, query)
    .no_redirects()
    .send()
    .await?;
  api.keep(&resp);
  if resp.status.is_redirection() {
    return Ok(resp);
  }
  resp.check()
}

/// The `zpData` of a passport answer, or its message as an error.
fn data(v: Value) -> Result<Value> {
  if code(&v) == Some(0) {
    return Ok(v.at("zpData").clone());
  }
  let message = v.str("message").unwrap_or_else(|| v.to_string());
  Err(Error::upstream(format!("BOSS Zhipin's login: {message}")))
}

pub async fn qr_start(api: &Api, scanned: &Cell<bool>) -> Result<QrTicket> {
  scanned.set(false);
  let v = passport(
    api,
    Method::POST,
    "/wapi/zppassport/captcha/randkey",
    &[],
    None,
  )
  .await?;
  let qr = data(v)?
    .str("qrId")
    .ok_or_else(|| Error::upstream("BOSS Zhipin's login gave no QR code"))?;
  let image = raw(
    api,
    "/wapi/zpweixin/qrcode/getqrcode",
    &[("content", qr.clone())],
  )
  .await?
  .body;
  if atp_core::file::sniff(&image).is_none() {
    return Err(Error::upstream(
      "the login QR code did not come as a picture",
    ));
  }
  Ok(QrTicket {
    token: qr,
    image: Some(image),
    ..QrTicket::default()
  })
}

/// A waiting call; no news within its time is no news.
async fn wait(api: &Api, path: &str, query: &[(&str, String)]) -> Result<Option<Value>> {
  match passport(api, Method::GET, path, query, Some(LONG_POLL)).await {
    Ok(v) => Ok(Some(v)),
    Err(e) if e.code == ErrorCode::NetworkError => Ok(None),
    Err(e) => Err(e),
  }
}

pub async fn qr_poll(api: &Api, ticket: &QrTicket, scanned: &Cell<bool>) -> Result<QrStatus> {
  let qr = ticket.token.clone();
  if !scanned.get() {
    let v = wait(api, "/wapi/zppassport/qrcode/scan", &[("uuid", qr)]).await?;
    if v.is_some_and(|v| v.bool("scaned") == Some(true)) {
      scanned.set(true);
      return Ok(QrStatus::Scanned);
    }
    return Ok(QrStatus::Waiting);
  }
  let query = [("qrId", qr.clone())];
  let v = wait(api, "/wapi/zppassport/qrcode/scanLogin", &query).await?;
  if !v.is_some_and(|v| v.bool("login") == Some(true)) {
    return Ok(QrStatus::Scanned);
  }
  // The dispatcher answers with the page to go on to; its cookies are
  // what counts.
  let query = [("qrId", qr), ("pk", "header-login".to_owned())];
  raw(api, "/wapi/zppassport/qrcode/dispatcher", &query).await?;
  if !api.logged_in() {
    return Err(Error::auth(
      "BOSS Zhipin confirmed the QR login but set no session cookies",
    ));
  }
  Ok(QrStatus::Confirmed)
}
