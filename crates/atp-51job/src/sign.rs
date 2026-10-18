//! The `sign` header the site's scripts put on every request to its APIs:
//! hex HMAC-SHA256 of the path and query (and the body, when there is
//! one), keyed with the site's key from its public scripts
//! (`commonStore.cupid_sign_key`).

use hmac::{Hmac, Mac};
use sha2::Sha256;

const KEY: &str = "abfc8f9dcf8c3f3d8aa294ac5f2cf2cc7767e5592590f39c3f503271dd68562b";

/// `sign` of a request to `path_and_query` with `body`.
pub fn sign(path_and_query: &str, body: &str) -> String {
  let mut mac = Hmac::<Sha256>::new_from_slice(KEY.as_bytes()).expect("any key length");
  mac.update(path_and_query.as_bytes());
  mac.update(body.as_bytes());
  mac
    .finalize()
    .into_bytes()
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect()
}
