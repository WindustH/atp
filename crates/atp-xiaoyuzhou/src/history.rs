//! The play history of the logged-in account (`list-history`), the
//! latest first, joined with how far each episode was played
//! (`playback-progress/list`), as the app's history page shows them.

use std::collections::HashMap;

use atp_core::{Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::{app, parse};

pub const HISTORY: &str = "/v1/episode-played/list-history";
const PROGRESS: &str = "/v1/playback-progress/list";

pub async fn history(api: &Api, page: &PageReq) -> Result<Page<Post>> {
  let app = api.app();
  let mut body = json!({});
  if let Some(cursor) = &page.cursor {
    body["loadMoreKey"] = app::load_more_key(cursor);
  }
  let v = app.post(HISTORY, &body).await?;
  // `{data: [...], loadMoreKey}`, or both one level down in `data`.
  let (rows, key) = match v.at("data") {
    Value::Array(rows) => (rows.as_slice(), v.at("loadMoreKey")),
    data => (data.list("data"), data.at("loadMoreKey")),
  };
  let episodes: Vec<&Value> = rows
    .iter()
    .map(|row| row.at("episode"))
    .filter(|e| e.str("eid").is_some())
    .collect();
  let eids: Vec<String> = episodes.iter().filter_map(|e| e.str("eid")).collect();
  let progress = if eids.is_empty() {
    Value::Null
  } else {
    app.post(PROGRESS, &json!({ "eids": eids })).await?
  };
  let played: HashMap<String, &Value> = progress
    .list("data")
    .iter()
    .filter_map(|p| Some((p.str("eid")?, p)))
    .collect();
  let items: Vec<Post> = episodes
    .into_iter()
    .map(|e| parse::played(e, e.str("eid").and_then(|id| played.get(&id).copied())))
    .collect();
  let next = app::next_cursor(key, items.len());
  Ok(Page::new(items, next))
}
