//! Users: profiles (`UserResource`), the search of people, and the
//! account of the login (its username from `UserSettingsResource`, then
//! its profile).

use atp_core::{Error, Page, PageReq, Query, Result, User, ValueExt, json};

use crate::api::Api;
use crate::{parse, pins, refs};

/// The most people a page asks for.
const PAGE: usize = 25;

async fn profile(api: &Api, name: &str) -> Result<User> {
  let options = json!({ "username": name, "field_set_key": "profile" });
  let answer = api
    .get("UserResource", options, &format!("/{name}/"))
    .await?;
  parse::profile(&answer.data)
    .ok_or_else(|| Error::not_found(format!("Pinterest has no user {name}")))
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  profile(api, &refs::user(arg)?).await
}

/// The username of the login.
pub async fn own_name(api: &Api) -> Result<String> {
  api.need_login()?;
  let answer = api
    .get("UserSettingsResource", json!({}), "/settings/")
    .await?;
  answer.data.str("username").ok_or_else(|| {
    Error::auth("Pinterest did not name the account of this session")
      .with_hint(api.ctx.login_hint())
  })
}

pub async fn whoami(api: &Api) -> Result<User> {
  let name = own_name(api).await?;
  profile(api, &name).await
}

pub async fn search(api: &Api, query: &Query, page: &PageReq) -> Result<Page<User>> {
  let (mut options, source) = pins::search_of(query, "users")?;
  options["page_size"] = json!(page.size_within(PAGE));
  if let Some(bookmark) = &page.cursor {
    options["bookmarks"] = json!([bookmark]);
  }
  let answer = api.get("BaseSearchResource", options, &source).await?;
  let rows = answer.rows();
  let users = rows
    .iter()
    .filter(|r| r.str("type").is_none_or(|t| t == "user"))
    .filter_map(parse::user)
    .collect();
  let next = answer.bookmark.clone().filter(|_| !rows.is_empty());
  Ok(Page::new(users, next))
}
