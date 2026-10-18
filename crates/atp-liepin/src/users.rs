//! Companies, which are Liepin's users here: a company's page
//! (`/company/<n>/`), following one (`comp.c.follow` / `unfollow`), and
//! the logged-in account (`usercx.pc.user.base-property`).

use atp_core::{Action, Error, Result, User, ValueExt};

use crate::api::{Api, USER};
use crate::{parse, refs};

pub async fn whoami(api: &Api) -> Result<User> {
  api.require_login()?;
  let v = api.form("usercx.pc.user.base-property", &[]).await?;
  let id = api.cookie(USER).unwrap_or_default();
  let name = v
    .first_str(&["userName", "userShowName", "name"])
    .unwrap_or_else(|| id.clone());
  Ok(User {
    avatar: v
      .first_str(&["photo", "userPhoto"])
      .as_deref()
      .and_then(parse::image),
    raw: Some(parse::sanitize(&v)),
    name,
    id,
    ..User::default()
  })
}

/// A company's page.
pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let id = refs::company(arg)?;
  let url = refs::company_url(&id);
  let html = api.page(&url).await?;
  parse::company(&html, &id)
    .ok_or_else(|| Error::not_found(format!("Liepin shows no company at {url}")))
}

/// Follow a company (关注), or stop.
pub async fn follow(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::company(arg)?;
  let url = refs::company_url(&id);
  let name = if undo {
    "comp.c.unfollow"
  } else {
    "comp.c.follow"
  };
  api.write(name, &[("compId", id.clone())], &url).await?;
  let action = if undo { "unfollow" } else { "follow" };
  Ok(Action::done(action, &id).with_url(url))
}
