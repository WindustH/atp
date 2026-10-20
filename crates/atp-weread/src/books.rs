//! The store: search, rankings (榜单) and categories, one book, its public
//! reviews (点评). With an API key, search, the book and its reviews come
//! from the Agent Gateway (`/store/search`, `/book/info`, `/review/list`).

use atp_core::{Comment, Error, Page, PageReq, Post, Query, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::gateway::Gateway;
use crate::parse;
use crate::refs;

/// `hot --category`: WeRead's rankings by a short name, with their ids.
const RANKINGS: &[(&str, &str)] = &[
  ("rising", "rising"),
  ("newbook", "newbook"),
  ("novel", "general_novel_rising"),
  ("all", "all"),
  ("newrating", "newrating"),
  ("potential", "potential"),
  ("hot-search", "hot_search"),
];
pub const RANKING_NAMES: &[&str] = &[
  "rising",
  "newbook",
  "novel",
  "all",
  "newrating",
  "potential",
  "hot-search",
];
/// `search --filter`: the gateway's search scopes.
const SCOPES: &[(&str, u64)] = &[
  ("all", 0),
  ("ebook", 10),
  ("webnovel", 16),
  ("audio", 14),
  ("author", 6),
  ("fulltext", 12),
  ("booklist", 13),
  ("mp", 2),
  ("article", 4),
];
pub const SCOPE_NAMES: &[&str] = &[
  "all", "ebook", "webnovel", "audio", "author", "fulltext", "booklist", "mp", "article",
];
/// `comments --sort`: the gateway's `reviewListType`.
const REVIEW_LISTS: &[(&str, u64)] = &[
  ("all", 0),
  ("recommend", 1),
  ("poor", 2),
  ("newest", 3),
  ("fair", 4),
];
pub const REVIEW_SORTS: &[&str] = &["all", "recommend", "poor", "newest", "fair"];
/// The web's list type of a book's public reviews.
const PUBLIC_REVIEWS: &str = "3";
/// Entries per search page.
const SEARCH_PAGE: usize = 20;

fn pick(table: &[(&str, u64)], name: &str, what: &str) -> Result<u64> {
  table
    .iter()
    .find(|(n, _)| *n == name)
    .map(|(_, code)| *code)
    .ok_or_else(|| Error::input(format!("unknown {what}: {name}")))
}

/// An option only the gateway knows, given without a key.
fn needs_key(option: &str) -> Error {
  Error::input(format!(
    "{option} needs WeRead's Agent Gateway: log in with `--token wrk-…` (or set WEREAD_API_KEY)"
  ))
}

pub async fn search(api: &Api, query: &Query, page: &PageReq) -> Result<Page<Post>> {
  if let Some(gw) = api.gateway() {
    return gateway_search(&gw, query, page).await;
  }
  if query.filter.as_deref().is_some_and(|f| f != "all") {
    return Err(needs_key("search --filter"));
  }
  let offset = page.number_or(0);
  let params = [
    ("keyword", query.keyword.clone()),
    ("maxIdx", offset.to_string()),
    ("count", page.size_within(SEARCH_PAGE).to_string()),
    ("fragmentSize", "120".to_owned()),
  ];
  let v = api.get("/web/search/global", &params).await?;
  let rows = v.list("books");
  let items: Vec<Post> = rows
    .iter()
    .enumerate()
    .map(|(i, row)| parse::ranked(row, offset + i as u64 + 1))
    .collect();
  let last = rows
    .last()
    .and_then(|r| r.u64("searchIdx"))
    .unwrap_or(offset + rows.len() as u64);
  Ok(Page::new(
    items,
    more(&v, rows.len()).then(|| last.to_string()),
  ))
}

/// `/store/search`: groups of results by scope; entries without a book
/// (authors, lists) are left out.
async fn gateway_search(gw: &Gateway<'_>, query: &Query, page: &PageReq) -> Result<Page<Post>> {
  let scope = pick(SCOPES, query.filter.as_deref().unwrap_or("all"), "scope")?;
  let offset = page.number_or(0);
  let count = page.size_within(SEARCH_PAGE);
  let v = gw
    .call(
      "/store/search",
      &[
        ("keyword", json!(query.keyword)),
        ("scope", json!(scope)),
        ("maxIdx", json!(offset)),
        ("count", json!(count)),
      ],
    )
    .await?;
  let rows: Vec<&Value> = v
    .list("results")
    .iter()
    .flat_map(|group| group.list("books"))
    .filter(|row| row.str("bookInfo.bookId").is_some())
    .collect();
  let items: Vec<Post> = rows
    .iter()
    .enumerate()
    .map(|(i, row)| parse::ranked(row, offset + i as u64 + 1))
    .collect();
  let last = rows.iter().filter_map(|r| r.u64("searchIdx")).max();
  let more = v.bool("hasMore").unwrap_or(rows.len() >= count);
  Ok(Page::new(
    items,
    last
      .filter(|_| more && !rows.is_empty())
      .map(|n| n.to_string()),
  ))
}

fn more(v: &Value, got: usize) -> bool {
  got > 0 && v.bool("hasMore").unwrap_or(false)
}

/// `hot`: a ranking (`rising` by default).
pub async fn hot(api: &Api, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  let name = category.unwrap_or("rising");
  let id = RANKINGS
    .iter()
    .find(|(n, _)| *n == name)
    .map(|(_, id)| *id)
    .ok_or_else(|| Error::input(format!("unknown ranking: {name}")))?;
  category_books(api, id, page).await
}

/// The books of a ranking or a store category (`100000` 精品小说 …), in
/// their order there.
pub async fn category_books(api: &Api, id: &str, page: &PageReq) -> Result<Page<Post>> {
  if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
    return Err(Error::input(format!(
      "not a WeRead category: {id} (give its id, such as 100000)"
    )));
  }
  let offset = page.number_or(0);
  let mut params = vec![("maxIndex", offset.to_string())];
  if !id.bytes().all(|b| b.is_ascii_digit()) {
    params.push(("rank", "1".into()));
  }
  let v = api
    .get(&format!("/web/bookListInCategory/{id}"), &params)
    .await?;
  let rows = v.list("books");
  let items: Vec<Post> = rows
    .iter()
    .enumerate()
    .map(|(i, row)| parse::ranked(row, offset + i as u64 + 1))
    .collect();
  let next = offset + rows.len() as u64;
  Ok(Page::new(
    items,
    more(&v, rows.len()).then(|| next.to_string()),
  ))
}

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::book(arg)?;
  if let Some(gw) = api.gateway() {
    let v = gw.call("/book/info", &[("bookId", json!(id))]).await?;
    return Ok(parse::book(&v));
  }
  api.require_login()?;
  let v = api.get("/web/book/info", &[("bookId", id)]).await?;
  Ok(parse::book(&v))
}

/// Public reviews (点评) of a book, in the order WeRead lists them; with
/// a key, also one kind of them (`--sort`).
pub async fn reviews(
  api: &Api,
  arg: &str,
  sort: Option<&str>,
  page: &PageReq,
) -> Result<Page<Comment>> {
  let id = refs::book(arg)?;
  if let Some(gw) = api.gateway() {
    return gateway_reviews(&gw, &id, sort, page).await;
  }
  if sort.is_some_and(|s| s != "all") {
    return Err(needs_key("comments --sort"));
  }
  api.require_login()?;
  let offset = page.number_or(0);
  let params = [
    ("bookId", id),
    ("listType", PUBLIC_REVIEWS.to_owned()),
    ("maxIdx", offset.to_string()),
    ("count", page.size_within(SEARCH_PAGE).to_string()),
    ("listMode", "2".to_owned()),
    ("synckey", "0".to_owned()),
  ];
  let v = api.get("/web/review/list", &params).await?;
  let rows = v.list("reviews");
  let items: Vec<Comment> = rows.iter().map(parse::review).collect();
  let next = rows
    .last()
    .and_then(|r| r.u64("idx"))
    .unwrap_or(offset + rows.len() as u64);
  let more = !rows.is_empty()
    && v
      .first_str(&["reviewsHasMore", "hasMore"])
      .is_some_and(|m| m == "1" || m == "true");
  Ok(Page::new(items, more.then(|| next.to_string())))
}

/// `/review/list`, paged by the last `idx` and the `synckey` it answered
/// with: the cursor is `IDX` or `IDX|SYNCKEY`.
async fn gateway_reviews(
  gw: &Gateway<'_>,
  id: &str,
  sort: Option<&str>,
  page: &PageReq,
) -> Result<Page<Comment>> {
  let list = pick(REVIEW_LISTS, sort.unwrap_or("all"), "review order")?;
  let (offset, synckey) = match page.cursor.as_deref().and_then(|c| c.split_once('|')) {
    Some((idx, key)) => (idx.parse().unwrap_or(0u64), json!(key)),
    None => (page.number_or(0), Value::Null),
  };
  let count = page.size_within(SEARCH_PAGE);
  let v = gw
    .call(
      "/review/list",
      &[
        ("bookId", json!(id)),
        ("reviewListType", json!(list)),
        ("count", json!(count)),
        ("maxIdx", json!(offset)),
        ("synckey", synckey),
      ],
    )
    .await?;
  let rows = v.list("reviews");
  let items: Vec<Comment> = rows.iter().map(parse::review).collect();
  let last = rows.last().and_then(|r| r.u64("idx"));
  let more = v
    .first_str(&["hasMore", "reviewsHasMore"])
    .map_or(rows.len() >= count, |m| m == "1" || m == "true");
  let next = last.filter(|_| more).map(|idx| match v.str("synckey") {
    Some(key) => format!("{idx}|{key}"),
    None => idx.to_string(),
  });
  Ok(Page::new(items, next))
}
