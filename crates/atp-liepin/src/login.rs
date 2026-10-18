//! The QR login of liepin.com's login box, scanned with the Liepin app:
//! `passport.qr.get-qrcode` hands out the code as a picture with a `key`,
//! `passport.qr.ack` (every 2 s, as the page polls) reports `10001`
//! waiting, `10006` scanned, `10003` / `10004` invalid or expired and `0`
//! confirmed, when the answer sets the session's cookies. Both go to
//! `api-passport.liepin.com` with the job seeker login's `businessId` and
//! `appId` and `keepLogin`, as the page sends them.

use atp_core::http::Method;
use atp_core::{Error, ErrorCode, QrStatus, QrTicket, Result, Value, ValueExt};
use base64::Engine;

use crate::api::Api;

/// The job seeker login of www.liepin.com.
const BUSINESS: &str = "1100100019";
const APP: &str = "6dp2Re4xQVd69gSD";

fn form(key: Option<&str>) -> Vec<(&'static str, String)> {
  let mut form = vec![
    ("businessId", BUSINESS.to_owned()),
    ("backurl", String::new()),
    ("appId", APP.to_owned()),
    ("keepLogin", "true".to_owned()),
  ];
  if let Some(key) = key {
    form.push(("key", key.to_owned()));
  }
  form
}

/// The `data` of a passport answer, or its message as an error.
fn data(v: &Value) -> Result<&Value> {
  if v.i64("flag") == Some(1) {
    return Ok(v.at("data"));
  }
  let message = v.str("msg").unwrap_or_else(|| v.to_string());
  Err(Error::upstream(format!("Liepin's login: {message}")))
}

/// The picture the QR code comes as: a `data:` URI or a link.
async fn picture(api: &Api, src: &str) -> Result<Vec<u8>> {
  if let Some((_, encoded)) = src.strip_prefix("data:").and_then(|s| s.split_once(",")) {
    return base64::engine::general_purpose::STANDARD
      .decode(encoded.trim())
      .map_err(|e| Error::upstream(format!("the login QR code is not a picture: {e}")));
  }
  let url = match src.strip_prefix("//") {
    Some(rest) => format!("https://{rest}"),
    None => src.to_owned(),
  };
  let resp = api
    .ctx
    .http
    .request(Method::GET, url)
    .no_cookies()
    .send()
    .await?
    .check()?;
  Ok(resp.body)
}

pub async fn qr_start(api: &Api) -> Result<QrTicket> {
  let v = api.passport("passport.qr.get-qrcode", &form(None)).await?;
  let data = data(&v)?;
  let (Some(src), Some(key)) = (data.str("qrcode"), data.str("key")) else {
    return Err(Error::upstream("Liepin's login gave no QR code"));
  };
  let image = picture(api, &src).await?;
  if atp_core::file::sniff(&image).is_none() {
    return Err(Error::upstream(
      "the login QR code did not come as a picture",
    ));
  }
  Ok(QrTicket {
    token: key,
    image: Some(image),
    ..QrTicket::default()
  })
}

pub async fn qr_poll(api: &Api, ticket: &QrTicket) -> Result<QrStatus> {
  let v = match api
    .passport("passport.qr.ack", &form(Some(&ticket.token)))
    .await
  {
    Ok(v) => v,
    Err(e) if e.code == ErrorCode::NetworkError => return Ok(QrStatus::Waiting),
    Err(e) => return Err(e),
  };
  let data = data(&v)?;
  Ok(match data.str("status").as_deref() {
    Some("0") => QrStatus::Confirmed,
    Some("10006") => QrStatus::Scanned,
    Some("10003" | "10004") => QrStatus::Expired,
    _ => QrStatus::Waiting,
  })
}
