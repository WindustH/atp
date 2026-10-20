//! The account's drafts (草稿) on the creator platform.

use atp_core::{Page, PageReq, Post, Result, ValueExt, paging};

use crate::api::Api;
use crate::creator::Creator;
use crate::parse;

const DRAFTS: &str = "/web/api/media/aweme/draft/";
const MANAGE_PAGE: &str = "https://creator.douyin.com/creator-micro/content/manage";

/// The drafts, all in one answer.
pub async fn list(api: &Api, page: &PageReq) -> Result<Page<Post>> {
  let creator = Creator::open(api).await?;
  let v = creator.get(DRAFTS).referer(MANAGE_PAGE).send().await?;
  let drafts = v
    .list("aweme_list")
    .iter()
    .filter_map(parse::draft)
    .collect();
  Ok(paging::slice(drafts, page))
}
