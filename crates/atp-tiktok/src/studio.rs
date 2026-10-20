//! TikTok Studio's content list (`/tiktokstudio/content`): your videos,
//! newest first, with the counters Studio shows for each (views, likes,
//! comments, saves, shares). The page reads it with a JSON POST to
//! `/tiktok/creator/manage/item_list/v1/`, cursor-paged.

use atp_core::{Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::parse;

const ITEM_LIST: &str = "/tiktok/creator/manage/item_list/v1/";
/// The page whose list this is: the referer of its calls.
const CONTENT_PAGE: &str = "https://www.tiktok.com/tiktokstudio/content";
/// The most videos one call answers with.
const PAGE_MAX: usize = 50;

/// One page of your videos, the newest first.
pub async fn videos(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let cursor: u64 = req
    .cursor
    .as_deref()
    .and_then(|c| c.parse().ok())
    .unwrap_or(0);
  let body = json!({
    "cursor": cursor,
    "size": req.size_within(PAGE_MAX),
    "query": {
      "conditions": [],
      "sort_orders": [{"field_name": "create_time", "order": 2}],
    },
  });
  let v = api.post_json(ITEM_LIST, &[], &body, CONTENT_PAGE).await?;
  // Some answers wrap the list in `data`.
  let data = if v.at("data").is_object() {
    v.at("data")
  } else {
    &v
  };
  let handle = api.ctx.extra("handle").unwrap_or_default();
  let items: Vec<Post> = data
    .list("item_list")
    .iter()
    .filter_map(|item| parse::studio_item(item, &handle))
    .collect();
  Ok(Page::new(items, next(data)))
}

/// The cursor of the next page, when Studio says there is one.
fn next(data: &Value) -> Option<String> {
  let more = data.bool("has_more").unwrap_or(false) && !data.list("item_list").is_empty();
  more.then(|| data.str("cursor")).flatten()
}
