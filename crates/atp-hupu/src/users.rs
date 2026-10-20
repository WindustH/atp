//! The account: whether the front page sees the session logged in
//! (`window.$$data.isLogin`), and the account's number from the `u`
//! cookie of the login (its first field). Profiles are not read: the
//! site shows them to logged-in readers only, and their data was not
//! identified.

use atp_core::{Error, Result, User, ValueExt};

use crate::api::Api;

pub async fn whoami(api: &Api) -> Result<User> {
  let no_login =
    || Error::auth("Hupu sees no login in this session").with_hint(api.ctx.login_hint());
  if !api.logged_in() {
    return Err(no_login());
  }
  let v = api.list_data("/").await?;
  if v.bool("isLogin") != Some(true) {
    return Err(no_login());
  }
  let cookie = api.ctx.http.cookie("u").unwrap_or_default();
  let cookie = atp_core::text::decode_uri_component(&cookie);
  let id = cookie
    .split('|')
    .next()
    .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    .unwrap_or_default()
    .to_owned();
  Ok(User {
    name: if id.is_empty() {
      "Hupu account".into()
    } else {
      id.clone()
    },
    id,
    ..User::default()
  })
}
