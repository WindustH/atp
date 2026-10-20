//! Shows: one show's page, and its episodes: the newest ones its page
//! lists, or, logged in, every episode page by page from the app's API
//! (`/v1/episode/list`).

use atp_core::paging::slice;
use atp_core::{Error, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::{app, parse};

const EPISODE_LIST: &str = "/v1/episode/list";
/// Episodes per page of the app's list.
const PAGE_MAX: usize = 20;

/// The show object of `/podcast/<id>`.
pub async fn data(api: &Api, id: &str) -> Result<Value> {
  let props = api.page(&format!("/podcast/{id}")).await?;
  let show = props.at("podcast");
  if show.str("pid").is_none() {
    return Err(Error::not_found(format!("Xiaoyuzhou has no show {id}")));
  }
  Ok(show.clone())
}

pub async fn read(api: &Api, id: &str) -> Result<Post> {
  let mut show = data(api, id).await?;
  if let Some(map) = show.as_object_mut() {
    map.remove("episodes");
  }
  Ok(parse::show(&show))
}

/// A show's episodes, the newest first.
pub async fn episodes(api: &Api, id: &str, page: &PageReq) -> Result<Page<Post>> {
  if api.app().logged_in() {
    return all_episodes(api, id, page).await;
  }
  let show = data(api, id).await?;
  let mut items: Vec<Post> = show.list("episodes").iter().map(parse::episode).collect();
  items.sort_by_key(|post| std::cmp::Reverse(post.created_at));
  Ok(slice(items, page))
}

/// One page of every episode of a show, from the app's API.
async fn all_episodes(api: &Api, id: &str, page: &PageReq) -> Result<Page<Post>> {
  let mut body = json!({
    "pid": id,
    "order": "desc",
    "limit": page.size_within(PAGE_MAX),
  });
  if let Some(cursor) = &page.cursor {
    body["loadMoreKey"] = app::load_more_key(cursor);
  }
  let v = api.app().post(EPISODE_LIST, &body).await?;
  let items: Vec<Post> = v.list("data").iter().map(parse::episode).collect();
  let next = app::next_cursor(v.at("loadMoreKey"), items.len());
  Ok(Page::new(items, next))
}
