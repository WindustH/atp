//! Users: the account behind the session (tRPC `user.me`, what the site's
//! pages ask; `null` without a login) and public profiles
//! (`/api/users/<uid>/profile`, which the API shows logged-in readers
//! only).

use atp_core::{Error, Result, User, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

pub async fn whoami(api: &Api) -> Result<User> {
  let me = api.trpc("user.me").await?;
  parse::me(&me).ok_or_else(|| {
    Error::auth("1Point3Acres sees no login in this session").with_hint(api.ctx.login_hint())
  })
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let uid = refs::user(arg)?;
  let v = api.get(&format!("/api/users/{uid}/profile"), &[]).await?;
  parse::profile(v.at("data"))
    .ok_or_else(|| Error::not_found(format!("1Point3Acres has no user {uid}")))
}
