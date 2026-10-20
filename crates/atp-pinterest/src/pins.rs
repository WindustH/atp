//! Pins: one pin (`PinResource`), the search of pins
//! (`BaseSearchResource`), a user's pins (`UserPinsResource`) and a
//! board's (`BoardFeedResource`). Listings go on with the bookmark of the
//! last page; ads and rows that are not pins are left out. The pins of
//! guests' lists are kept for `read` and `download` ([`listed`]).

use atp_core::{Error, ErrorCode, Page, PageReq, Post, Query, Result, Value, ValueExt, json, text};

use crate::api::Api;
use crate::refs::{self, PinRef};
use crate::{boards, listed, parse};

/// The most rows a page asks for.
const PAGE: usize = 25;

/// A pin's id; a `pin.it` link is followed to its pin.
pub async fn id(api: &Api, arg: &str) -> Result<String> {
  match refs::pin(arg)? {
    PinRef::Id(id) => Ok(id),
    PinRef::Short(link) => {
      let target = api.ctx.http.final_url(&link).await?;
      refs::pin_of_link(&target)
        .ok_or_else(|| Error::not_found(format!("{link} does not lead to a pin ({target})")))
    }
  }
}

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = id(api, arg).await?;
  if !listed::guest_id(&id) {
    return pin(api, &id).await;
  }
  if let Some(row) = listed::row(api, &id) {
    return Ok(parse::pin(&row));
  }
  pin(api, &id).await.map_err(|e| {
    if e.code != ErrorCode::NotFound {
      return e;
    }
    Error::auth(format!(
      "pin {id} has an id Pinterest gives guests' lists, which opens only within the visit that listed it, and ATP no longer keeps it"
    ))
    .with_hint("list it again (`search`), or log in: `atp guide login pinterest`")
  })
}

/// A pin from `PinResource`, with the fields the site shows guests or the
/// detailed ones.
async fn pin(api: &Api, id: &str) -> Result<Post> {
  let fields = if api.logged_in() {
    "detailed"
  } else {
    "unauth_react_main_pin"
  };
  let options = json!({ "id": id, "field_set_key": fields });
  let answer = api
    .get("PinResource", options, &format!("/pin/{id}/"))
    .await?;
  if answer.data.str("id").is_none() {
    return Err(Error::not_found(format!("Pinterest has no pin {id}")));
  }
  Ok(parse::pin(&answer.data))
}

/// Whether a row of a listing is a pin, and not an ad.
fn is_pin(row: &Value) -> bool {
  row.str("id").is_some()
    && row.str("type").is_none_or(|t| t == "pin")
    && row.bool("is_promoted") != Some(true)
}

/// A page of a listing of pins.
async fn listing(
  api: &Api,
  resource: &str,
  mut options: Value,
  source: &str,
  page: &PageReq,
) -> Result<Page<Post>> {
  options["page_size"] = json!(page.size_within(PAGE));
  if let Some(bookmark) = &page.cursor {
    options["bookmarks"] = json!([bookmark]);
  }
  let answer = api.get(resource, options, source).await?;
  let rows = answer.rows();
  let pins: Vec<&Value> = rows.iter().filter(|r| is_pin(r)).collect();
  listed::remember(api, &pins);
  let posts = pins.into_iter().map(parse::pin).collect();
  let next = answer.bookmark.clone().filter(|_| !rows.is_empty());
  Ok(Page::new(posts, next))
}

/// The search page of a scope (`pins`, `boards`, `users`) and the options
/// of its search.
pub fn search_of(query: &Query, scope: &str) -> Result<(Value, String)> {
  let word = query.keyword.trim();
  if word.is_empty() {
    return Err(Error::input("search for something"));
  }
  let source = format!(
    "/search/{scope}/?q={}&rs=typed",
    text::encode_uri_component(word)
  );
  let options = json!({
    "query": word,
    "scope": scope,
    "rs": "typed",
    "auto_correction_disabled": false,
    "appliedProductFilters": "---",
    "source_url": source,
  });
  Ok((options, source))
}

pub async fn search(api: &Api, query: &Query, page: &PageReq) -> Result<Page<Post>> {
  let (options, source) = search_of(query, "pins")?;
  listing(api, "BaseSearchResource", options, &source, page).await
}

/// A user's pins, as their profile lists them.
pub async fn by_user(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  let name = refs::user(arg)?;
  let options = json!({ "username": name, "field_set_key": "grid_item" });
  listing(api, "UserPinsResource", options, &format!("/{name}/"), page).await
}

/// The pins of a board.
pub async fn on_board(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  let board = boards::find(api, arg).await?;
  let (id, path) = (
    board.str("id").unwrap_or_default(),
    board.str("url").unwrap_or_default(),
  );
  let options = json!({
    "board_id": id,
    "board_url": path,
    "currentFilter": -1,
    "field_set_key": "react_grid_pin",
    "filter_section_pins": true,
    "sort": "default",
    "layout": "default",
  });
  listing(api, "BoardFeedResource", options, &path, page).await
}
