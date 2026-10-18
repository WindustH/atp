//! The logged-in account (`/profile/user-info-v2`, then its profile
//! `/api/sparta/user/profile/<id>`) and following users (`/sns/follow`,
//! `/sns/unfollow`).

use atp_core::{Action, Error, Result, User, ValueExt};

use crate::api::{Api, Body, Route};
use crate::{parse, refs};

/// The logged-in account's number.
pub async fn uid(api: &Api) -> Result<String> {
  api.require_login()?;
  let info = api.get(Route::Site, "/profile/user-info-v2", &[]).await?;
  info
    .first_str(&["userId", "uid", "id"])
    .filter(|id| id != "0" && id != "-1")
    .ok_or_else(|| {
      Error::auth("Nowcoder did not name the logged-in account").with_hint(api.ctx.login_hint())
    })
}

pub async fn whoami(api: &Api) -> Result<User> {
  let id = uid(api).await?;
  let profile = api
    .get(
      Route::Gateway,
      &format!("/api/sparta/user/profile/{id}"),
      &[],
    )
    .await?;
  Ok(parse::user(&profile).unwrap_or_else(|| User {
    url: Some(refs::user_url(&id)),
    name: id.clone(),
    id,
    ..User::default()
  }))
}

/// Follow a user (关注) or stop.
pub async fn follow(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::user(arg)?;
  let path = if undo { "/sns/unfollow" } else { "/sns/follow" };
  let form = [
    ("id", id.clone()),
    ("type", "1".to_owned()),
    ("reason", "basic".to_owned()),
    ("source", "home".to_owned()),
  ];
  api.write(Route::Site, path, &[], Body::Form(&form)).await?;
  let action = if undo { "unfollow" } else { "follow" };
  Ok(Action::done(action, &id).with_url(refs::user_url(&id)))
}
