//! Inside a book, without its text: the table of contents (the gateway's
//! `/book/chapterinfo`, else the web reader's `/web/book/chapterInfos`),
//! WeRead's AI outline of the chapters (`/web/book/outline`), and the
//! places a word occurs (`/web/book/search`), with a few words around
//! each.

use std::collections::HashMap;

use atp_core::{Error, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::refs;

/// Matches per page of the in-book search.
const SEARCH_PAGE: usize = 50;
/// Characters around each match.
const FRAGMENT: u32 = 150;

/// The chapters of a book, in order: index, uid, title, level, words and
/// price (yuan; 0 when free), as the gateway or the reader lists them.
pub async fn chapters(api: &Api, arg: &str) -> Result<Vec<Value>> {
  let id = refs::book(arg)?;
  let rows = match api.gateway() {
    Some(gw) => gw
      .call("/book/chapterinfo", &[("bookId", json!(id))])
      .await?
      .list("chapters")
      .to_vec(),
    None => {
      api.require_login()?;
      let body = json!({"bookIds": [id], "sinces": [0]});
      let v = api.post("/web/book/chapterInfos", &body).await?;
      v.list("data.0.updated").to_vec()
    }
  };
  Ok(rows.iter().map(chapter).collect())
}

fn chapter(c: &Value) -> Value {
  json!({
    "index": c.u64("chapterIdx"),
    "uid": c.u64("chapterUid"),
    "title": c.str("title"),
    "level": c.u64("level"),
    "words": c.u64("wordCount"),
    "price": c.f64("price").filter(|p| *p > 0.0),
    "paid": c.bool("paid").filter(|p| *p),
  })
}

/// WeRead's AI outline of a book: one row per point, with its chapter,
/// its number (`2.1` …) and its level (2 topics, 3 key points, 4
/// details), down to `depth`.
pub async fn outline(api: &Api, arg: &str, depth: u64) -> Result<Vec<Value>> {
  let id = refs::book(arg)?;
  let toc = chapters(api, &id).await?;
  let titles: HashMap<u64, String> = toc
    .iter()
    .filter_map(|c| Some((c.u64("uid")?, c.str("title").unwrap_or_default())))
    .collect();
  let uids: Vec<u64> = toc.iter().filter_map(|c| c.u64("uid")).collect();
  if uids.is_empty() {
    return Err(Error::not_found(format!("book {id} lists no chapters")));
  }
  let v = api
    .post(
      "/web/book/outline",
      &json!({"bookId": id, "chapterUids": uids}),
    )
    .await?;
  let mut rows = Vec::new();
  for part in v.list("itemsArray") {
    let items = part.list("items");
    // The chapter: by its uid (also in each point's `uniqId`,
    // `book-uid-…`), else the outline's own first line (level 1).
    let uid = part.u64("chapterUid").or_else(|| {
      let first = items.first()?.str("uniqId")?;
      first.split('-').nth(1)?.parse().ok()
    });
    let chapter = uid.and_then(|u| titles.get(&u).cloned()).or_else(|| {
      items
        .iter()
        .find(|i| i.u64("level") == Some(1))
        .and_then(|i| i.str("text"))
    });
    rows.extend(points(items, chapter.as_deref(), depth));
  }
  if rows.is_empty() {
    return Err(Error::not_found(format!(
      "WeRead has no AI outline of book {id}"
    )));
  }
  Ok(rows)
}

/// The points of one chapter's outline; details (level 4) without a
/// number are numbered under the key point before them.
fn points(items: &[Value], chapter: Option<&str>, depth: u64) -> Vec<Value> {
  let (mut point, mut detail) = (String::new(), 0);
  let mut rows = Vec::new();
  for item in items {
    let level = item.u64("level").unwrap_or(1);
    if level <= 1 || level > depth {
      continue;
    }
    let mut number = item.str("uiIdx").unwrap_or_default();
    if level == 3 && !number.is_empty() {
      (point, detail) = (number.clone(), 0);
    }
    if level == 4 && number.is_empty() && !point.is_empty() {
      detail += 1;
      number = format!("{point}.{detail}");
    }
    rows.push(json!({
      "chapter": chapter,
      "number": number,
      "level": level,
      "text": item.str("text"),
    }));
  }
  rows
}

/// The places `keyword` occurs in a book, in reading order: posts of kind
/// `match` whose text is the passage around it; paged by `searchIdx`.
pub async fn search(api: &Api, arg: &str, keyword: &str, page: &PageReq) -> Result<Page<Post>> {
  let id = refs::book(arg)?;
  let offset = page.number_or(0);
  let params = [
    ("bookId", id.clone()),
    ("keyword", keyword.to_owned()),
    ("maxIdx", offset.to_string()),
    ("count", page.size_within(SEARCH_PAGE).to_string()),
    ("fragmentSize", FRAGMENT.to_string()),
    ("onlyCount", "0".to_owned()),
  ];
  let v = api.get("/web/book/search", &params).await?;
  let rows = v.list("result");
  let items: Vec<Post> = rows.iter().filter_map(|r| found(&id, r)).collect();
  let last = rows.iter().filter_map(|r| r.u64("searchIdx")).max();
  let more = v.bool("hasMore").unwrap_or(false);
  Ok(Page::new(
    items,
    last.filter(|n| more && *n > offset).map(|n| n.to_string()),
  ))
}

fn found(book: &str, v: &Value) -> Option<Post> {
  let place = v.u64("searchIdx")?;
  let text = v.str("abstract")?;
  let mut post = Post {
    id: format!("{book}:{place}"),
    kind: "match".into(),
    text: Some(atp_core::text::html_to_text(&text)),
    url: Some(refs::book_url(book)),
    raw: Some(v.clone()),
    ..Post::default()
  };
  for (key, value) in [
    ("book", json!(book)),
    ("chapter_uid", json!(v.u64("chapterUid"))),
    ("chapter_index", json!(v.u64("chapterIdx"))),
    ("search_idx", json!(place)),
  ] {
    if !value.is_null() {
      post.extra.insert(key.into(), value);
    }
  }
  Some(post)
}
