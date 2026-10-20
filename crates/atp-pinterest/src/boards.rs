//! Boards: a user's boards (`BoardsResource`), the search of boards, one
//! board by its path or number (`BoardResource`), and creating one with a
//! login.

use atp_core::{
  Action, Collection, Error, FolderSpec, Page, PageReq, Query, Result, Value, ValueExt, json,
};

use crate::api::{Api, SITE};
use crate::refs::{self, BoardRef};
use crate::{parse, pins, users};

/// The most boards a page asks for.
const PAGE: usize = 25;

/// A page of a listing of boards.
async fn listing(
  api: &Api,
  resource: &str,
  mut options: Value,
  source: &str,
  page: &PageReq,
) -> Result<Page<Collection>> {
  options["page_size"] = json!(page.size_within(PAGE));
  if let Some(bookmark) = &page.cursor {
    options["bookmarks"] = json!([bookmark]);
  }
  let answer = api.get(resource, options, source).await?;
  let rows = answer.rows();
  let boards = rows
    .iter()
    .filter(|r| r.str("id").is_some() && r.str("type").is_none_or(|t| t == "board"))
    .map(parse::board)
    .collect();
  let next = answer.bookmark.clone().filter(|_| !rows.is_empty());
  Ok(Page::new(boards, next))
}

/// A user's boards (yours without a user).
pub async fn list(api: &Api, user: Option<&str>, page: &PageReq) -> Result<Page<Collection>> {
  let name = match user {
    Some(user) => refs::user(user)?,
    None => users::own_name(api).await?,
  };
  let options = json!({
    "username": name,
    "sort": "last_pinned_to",
    "privacy_filter": "all",
    "field_set_key": "profile_grid_item",
  });
  listing(api, "BoardsResource", options, &format!("/{name}/"), page).await
}

pub async fn search(api: &Api, query: &Query, page: &PageReq) -> Result<Page<Collection>> {
  let (options, source) = pins::search_of(query, "boards")?;
  listing(api, "BaseSearchResource", options, &source, page).await
}

/// A board's data (`id`, `url` …) from its path, link or number, kept for
/// the pages of one listing.
pub async fn find(api: &Api, arg: &str) -> Result<Value> {
  if let Some(found) = api.boards.borrow().get(arg) {
    return Ok(found.clone());
  }
  let board = refs::board(arg)?;
  let (options, source) = match &board {
    BoardRef::Path { user, slug } => (
      json!({ "username": user, "slug": slug, "field_set_key": "detailed" }),
      format!("/{user}/{slug}/"),
    ),
    BoardRef::Id(id) => (
      json!({ "board_id": id, "field_set_key": "detailed" }),
      "/".to_owned(),
    ),
  };
  let answer = api.get("BoardResource", options, &source).await?;
  if answer.data.str("id").is_none() || answer.data.str("url").is_none() {
    return Err(Error::not_found(format!("Pinterest has no board {arg}")));
  }
  api
    .boards
    .borrow_mut()
    .insert(arg.to_owned(), answer.data.clone());
  Ok(answer.data)
}

/// Create a board (`folder-create`): public, or secret with `--private`.
pub async fn create(api: &Api, folder: &FolderSpec) -> Result<Action> {
  let name = folder.name.clone().unwrap_or_default();
  let mut options = json!({
    "name": name,
    "privacy": if folder.private == Some(true) { "secret" } else { "public" },
  });
  if let Some(description) = &folder.description {
    options["description"] = json!(description);
  }
  let data = api.act("BoardResource", "create", options, "/").await?;
  let mut action = Action::done("folder-create", name);
  if let Some(id) = parse::board_path(&data).or_else(|| data.str("id")) {
    action = action.with_id(id);
  }
  if let Some(url) = data.str("url") {
    action = action.with_url(format!("{SITE}{url}"));
  }
  Ok(action)
}
