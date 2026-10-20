//! Books WeRead suggests, from the Agent Gateway only: recommendations
//! for you (`/book/recommend`, for `feed`) and books like one
//! (`/book/similar`, for `related`).

use atp_core::{Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::{parse, refs};

/// Books per page.
const PAGE_MAX: usize = 20;

/// Recommendations, paged by the last `searchIdx`; each with WeRead's
/// reason (`extra.reason`).
pub async fn recommended(api: &Api, page: &PageReq) -> Result<Page<Post>> {
  let gw = api.require_gateway("feed (recommendations)")?;
  let count = page.size_within(PAGE_MAX);
  let offset = page.number_or(0);
  let v = gw
    .call(
      "/book/recommend",
      &[("count", json!(count)), ("maxIdx", json!(offset))],
    )
    .await?;
  let rows = v.list("books");
  let items: Vec<Post> = rows
    .iter()
    .enumerate()
    .map(|(i, row)| {
      let mut post = parse::ranked_flat(row, offset + i as u64 + 1);
      if let Some(reason) = row.str("reason").filter(|r| !r.is_empty()) {
        post.extra.insert("reason".into(), json!(reason));
      }
      post
    })
    .collect();
  let last = rows.iter().filter_map(|r| r.u64("searchIdx")).max();
  let more = v.bool("hasMore").unwrap_or(rows.len() >= count);
  Ok(Page::new(
    items,
    last.filter(|_| more).map(|n| n.to_string()),
  ))
}

/// Books like one, paged by the last `idx` and the gateway's session id:
/// the cursor is `IDX|SESSION`.
pub async fn similar(api: &Api, book: &str, page: &PageReq) -> Result<Page<Post>> {
  let gw = api.require_gateway("related (similar books)")?;
  let id = refs::book(book)?;
  let (offset, session) = match page.cursor.as_deref().and_then(|c| c.split_once('|')) {
    Some((idx, session)) => (idx.parse().unwrap_or(0u64), json!(session)),
    None => (0, Value::Null),
  };
  let count = page.size_within(PAGE_MAX);
  let v = gw
    .call(
      "/book/similar",
      &[
        ("bookId", json!(id)),
        ("count", json!(count)),
        ("maxIdx", json!(offset)),
        ("sessionId", session),
      ],
    )
    .await?;
  let inner = if v.at("booksimilar").is_object() {
    v.at("booksimilar")
  } else {
    &v
  };
  let rows = inner.list("books");
  let items: Vec<Post> = rows
    .iter()
    .enumerate()
    .map(|(i, row)| {
      let info = [row.at("book.bookInfo"), row.at("bookInfo")]
        .into_iter()
        .find(|b| b.is_object())
        .unwrap_or(row);
      parse::ranked_flat(info, row.u64("idx").unwrap_or(offset + i as u64 + 1))
    })
    .collect();
  let last = rows.iter().filter_map(|r| r.u64("idx")).max();
  let more = inner.bool("hasMore").unwrap_or(rows.len() >= count);
  let session = inner.str("sessionId").unwrap_or_default();
  Ok(Page::new(
    items,
    last.filter(|_| more).map(|n| format!("{n}|{session}")),
  ))
}
