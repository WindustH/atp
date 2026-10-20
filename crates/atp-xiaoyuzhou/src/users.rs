//! Users: a user's page (profile, counts, the shows they host), the
//! logged-in account (from the app's API), and the posts of a show or
//! user for `user-posts`: a show's episodes, or the shows a user hosts.

use atp_core::{Error, ErrorCode, Page, PageReq, Post, Result, User, ValueExt, json};

use crate::api::Api;
use crate::history::HISTORY;
use crate::refs::{self, Item};
use crate::{parse, shows};

const PROFILE: &str = "/v1/profile/get";

/// The logged-in account: its profile, as the app's "me" page reads it.
/// When that answer names no account, the session is checked against the
/// play history instead, and the account goes unnamed.
pub async fn whoami(api: &Api) -> Result<User> {
  let app = api.app();
  app.require_login()?;
  let found = match app.get(PROFILE, &[]).await {
    Ok(v) => Some(v),
    Err(e)
      if matches!(
        e.code,
        ErrorCode::NotFound | ErrorCode::UpstreamError | ErrorCode::InvalidInput
      ) =>
    {
      None
    }
    Err(e) => return Err(e),
  };
  let data = found.as_ref().map(|v| v.at("data"));
  match data {
    Some(d) if d.str("user.uid").is_some() => return Ok(parse::profile(d)),
    Some(d) if d.str("uid").is_some() => return Ok(parse::user(d)),
    _ => {}
  }
  app.post(HISTORY, &json!({})).await?;
  Ok(User {
    name: "(the logged-in account; its profile was not read)".into(),
    ..User::default()
  })
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  match refs::resolve(api, arg).await? {
    Item::User(id) | Item::Bare(id) => profile(api, &id).await,
    Item::Show(_) => Err(Error::input(
      "that is a show; `read` it (its hosts are in extra.podcasters)",
    )),
    Item::Episode(_) => Err(Error::input("that is an episode; use `read`")),
  }
}

async fn profile(api: &Api, id: &str) -> Result<User> {
  let props = api.page(&format!("/user/{id}")).await?;
  let profile = props.at("profile");
  if profile.str("user.uid").is_none() {
    return Err(Error::not_found(format!("Xiaoyuzhou has no user {id}")));
  }
  Ok(parse::profile(profile))
}

/// A show's episodes, or the shows a user hosts; a bare id is tried as a
/// show, then as a user.
pub async fn posts(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  match refs::resolve(api, arg).await? {
    Item::Show(id) => shows::episodes(api, &id, page).await,
    Item::User(id) => hosted(api, &id).await,
    Item::Episode(_) => Err(Error::input("that is an episode; give its show")),
    Item::Bare(id) => bare_posts(api, &id, page).await,
  }
}

/// A bare id as a show, else as a user. Logged in, the show's page is
/// looked up first: the app's episode list does not say a show is missing.
async fn bare_posts(api: &Api, id: &str, page: &PageReq) -> Result<Page<Post>> {
  if api.app().logged_in()
    && let Err(e) = shows::data(api, id).await
  {
    return match e.code {
      ErrorCode::NotFound => hosted(api, id).await,
      _ => Err(e),
    };
  }
  match shows::episodes(api, id, page).await {
    Err(e) if e.code == ErrorCode::NotFound => hosted(api, id).await,
    other => other,
  }
}

/// The shows a user hosts.
async fn hosted(api: &Api, id: &str) -> Result<Page<Post>> {
  let props = api.page(&format!("/user/{id}")).await?;
  let shows = props.list("profile.ownedPodcasts");
  Ok(Page::last(shows.iter().map(parse::show).collect()))
}
