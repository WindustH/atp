//! Creator numbers of the account's works: the creator platform's works
//! list (`creator/item/list`, which its manage and data pages read) with
//! each work's metrics, review state and visibility, and `insights POST`,
//! the row of one work found in it.

use atp_core::{Error, Insights, Page, PageReq, Post, Result, Value, ValueExt};

use crate::api::Api;
use crate::creator::Creator;
use crate::{parse, refs};

const WORKS: &str = "/web/api/creator/item/list";
const MANAGE_PAGE: &str = "https://creator.douyin.com/creator-micro/content/manage";
/// The groups of fields a row carries: the work itself and its metrics,
/// review state and visibility.
const FIELDS: &str = "default,metrics,review,visibility";
/// Rows per page, the most other clients ask for.
const PAGE_SIZE: usize = 50;
/// Pages `insights POST` looks through: the latest 1000 works.
const SEARCH_PAGES: usize = 20;
const ACCOUNT_HINT: &str = "give one of your works: `atp douyin insights POST`; \
   `atp douyin works` lists them all with their numbers";

/// The account's works, latest first, with their creator numbers.
pub async fn works(api: &Api, page: &PageReq) -> Result<Page<Post>> {
  let creator = Creator::open(api).await?;
  let cursor = page.cursor.as_deref();
  let v = works_page(&creator, cursor, page.size_within(PAGE_SIZE)).await?;
  Ok(parse::works(&v, cursor))
}

/// `insights`: one of the account's works; the account as a whole is not
/// offered.
pub async fn insights(api: &Api, post: Option<&str>) -> Result<Insights> {
  match post {
    Some(post) => work(api, post).await,
    None => Err(Error::unsupported("insights without POST").with_hint(ACCOUNT_HINT)),
  }
}

/// The numbers of one work, from its row in the works list.
async fn work(api: &Api, post: &str) -> Result<Insights> {
  let id = refs::aweme(api, post).await?;
  let creator = Creator::open(api).await?;
  let mut cursor: Option<String> = None;
  for _ in 0..SEARCH_PAGES {
    let v = works_page(&creator, cursor.as_deref(), PAGE_SIZE).await?;
    let row = v
      .list("items")
      .iter()
      .find(|w| parse::work_id(w).as_deref() == Some(id.as_str()));
    if let Some(insights) = row.and_then(parse::work_insights) {
      return Ok(insights);
    }
    match parse::works_cursor(&v, cursor.as_deref()) {
      Some(next) => cursor = Some(next),
      None => break,
    }
  }
  Err(Error::not_found(format!(
    "{id} is not among your latest {} works on creator.douyin.com (insights cover your own works)",
    SEARCH_PAGES * PAGE_SIZE
  )))
}

async fn works_page(creator: &Creator<'_>, cursor: Option<&str>, count: usize) -> Result<Value> {
  let mut call = creator
    .get(WORKS)
    .arg("count", count)
    .arg("order_by", 1)
    .arg("fields", FIELDS)
    .arg("need_cooperation", "true")
    .arg("need_long_article", "true");
  if let Some(cursor) = cursor {
    call = call.arg("max_cursor", cursor);
  }
  call.referer(MANAGE_PAGE).send().await
}
