//! The headers the web app (wx.zsxq.com) puts on every API request, from
//! its public scripts:
//!
//! - `X-Request-Id`: a random id of 32 hex digits, dashes after the 9th,
//!   13th, 17th and 21st;
//! - `X-Timestamp`: Unix seconds;
//! - `X-Signature`: the SHA-1 (hex) of `<url> <timestamp> <request id>`,
//!   the URL as sent, with `'` in its query written `%27`. No key goes
//!   into it; it is a checksum of the request, not a token about the
//!   browser;
//! - `X-Version`: the app's API version, another one for `/v3/` paths;
//! - `X-Aduid`: an id of the browser in the same form as the request id,
//!   which the app keeps in its local storage; ATP keeps one with the
//!   session.
//!
//! Without them `/v3/` endpoints answer code 1059 (设备状态异常).

use rand::Rng;
use sha1::{Digest, Sha1};

/// `X-Version` of `/v2/` requests.
const X_VERSION: &str = "2.98.0";
/// `X-Version` of `/v3/` requests.
const V3_X_VERSION: &str = "3.24.0";

/// The signing headers of one request to `url` (the whole URL, as sent).
pub fn headers(url: &str, aduid: &str) -> Vec<(&'static str, String)> {
  let timestamp = jiff::Timestamp::now().as_second().to_string();
  let request_id = random_id();
  let version = if url.contains("/v3/") {
    V3_X_VERSION
  } else {
    X_VERSION
  };
  vec![
    ("x-request-id", request_id.clone()),
    ("x-version", version.to_owned()),
    ("x-signature", signature(url, &timestamp, &request_id)),
    ("x-timestamp", timestamp),
    ("x-aduid", aduid.to_owned()),
  ]
}

/// SHA-1 of `<url> <timestamp> <request id>`, as the app computes it.
fn signature(url: &str, timestamp: &str, request_id: &str) -> String {
  let url = match url.split_once('?') {
    Some((path, query)) => format!("{path}?{}", query.replace('\'', "%27")),
    None => url.to_owned(),
  };
  let digest = Sha1::digest(format!("{url} {timestamp} {request_id}").as_bytes());
  digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 32 random hex digits with dashes after the 9th, 13th, 17th and 21st,
/// the app's form of request and browser ids.
pub fn random_id() -> String {
  let mut rng = rand::rng();
  let mut id = String::with_capacity(36);
  for i in 0..32 {
    let digit = rng.random_range(0..16u32);
    id.push(char::from_digit(digit, 16).unwrap_or('0'));
    if matches!(i, 8 | 12 | 16 | 20) {
      id.push('-');
    }
  }
  id
}
