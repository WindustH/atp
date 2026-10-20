//! Users: you, and other readers by their number.

use atp_core::{Error, Result, User, json};

use crate::api::Api;
use crate::parse;
use crate::refs;

/// You: the cookie session's reader, else the API key's owner.
pub async fn whoami(api: &Api) -> Result<User> {
  if !api.has_cookies() && api.gateway().is_some() {
    return key_owner(api).await;
  }
  api.require_login()?;
  let vid = api
    .vid()
    .ok_or_else(|| Error::auth("the session has no wr_vid cookie"))?;
  user(api, &vid).await
}

/// The owner of the API key, checked with a one-book notebook call. The
/// gateway does not name its reader, so the account goes by its key.
pub async fn key_owner(api: &Api) -> Result<User> {
  let gw = api.require_gateway("whoami")?;
  gw.call("/user/notebooks", &[("count", json!(1))]).await?;
  Ok(User {
    id: api.vid().unwrap_or_default(),
    name: format!("WeRead API key {}", gw.masked_key()),
    ..User::default()
  })
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let vid = refs::user(arg)?;
  api.require_login()?;
  let v = api.get("/web/user", &[("userVid", vid.clone())]).await?;
  let mut user = parse::user(&v);
  if user.id.is_empty() {
    user.id = vid;
  }
  Ok(user)
}
