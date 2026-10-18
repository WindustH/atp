//! Users (`/users/<id>`) and the logged-in account (`/profile/info`).

use atp_core::{Error, Result, User, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

pub async fn whoami(api: &Api) -> Result<User> {
  api.require_login()?;
  let v = api.get("/profile/info", &[]).await?;
  let user = v.at("user");
  if user.str("id").is_none() {
    return Err(
      Error::auth("Eleduck did not name the logged-in account").with_hint(api.ctx.login_hint()),
    );
  }
  Ok(parse::profile(user))
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let id = refs::user(arg)?;
  let v = api.get(&format!("/users/{id}"), &[]).await?;
  let user = v.at("user");
  if user.str("id").is_none() {
    return Err(Error::not_found(format!("Eleduck has no user {id}")));
  }
  Ok(parse::profile(user))
}
