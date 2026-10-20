//! Locations (POI, 位置) a post can be tagged with, found as the creator
//! platform's publish form finds them (`life/video_api/search/poi`).

use atp_core::{Collection, Page, PageReq, Result, ValueExt};

use crate::api::Api;
use crate::creator::Creator;
use crate::parse;

const SEARCH: &str = "/aweme/v1/life/video_api/search/poi/";
/// The location box's page size.
const COUNT: u64 = 12;
/// The box's region tab: 0 local (around the browser's position, which
/// ATP does not have), 2 domestic (国内), 3 abroad.
const DOMESTIC: u64 = 2;
/// The location tab of the box (`poi_anchor_tab`).
const LOCATION_TAB: u64 = 2;

/// Locations in mainland China matching `keyword`. Pages count from 1;
/// the answer's `page` is the next one.
pub async fn search(api: &Api, keyword: &str, page: &PageReq) -> Result<Page<Collection>> {
  let creator = Creator::open(api).await?;
  let number = page.number_or(1);
  let v = creator
    .get(SEARCH)
    .arg("count", COUNT)
    .arg("from_webapp", 1)
    .arg("keywords", keyword.trim())
    .arg("search_type", DOMESTIC)
    .arg("poi_anchor_tab", LOCATION_TAB)
    .arg("page", number)
    .send()
    .await?;
  let items: Vec<Collection> = v.list("poi_list").iter().filter_map(parse::poi).collect();
  let more = v.bool("has_more") == Some(true) && !items.is_empty();
  let next = v.u64("page").filter(|p| *p > number).unwrap_or(number + 1);
  Ok(Page::new(items, more.then(|| next.to_string())))
}
