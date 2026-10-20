//! The inbox's activity (likes, comments, mentions, followers) of the
//! logged-in account: `/api/notice/multi/` by `notice_type`, paged
//! backwards in time by `max_time`.

use atp_core::{Error, Notification, Page, PageReq, Result, Value, ValueExt};

use crate::api::Api;
use crate::parse;

const NOTICE: &str = "/api/notice/multi/";
/// The most notices one call asks for.
const PAGE_MAX: usize = 30;

pub const KINDS: &[&str] = &["all", "like", "comment", "mention", "follower"];

/// `notice_type` of each kind.
fn notice_type(kind: Option<&str>) -> Result<&'static str> {
  Ok(match kind.unwrap_or("all") {
    "all" => "0",
    "like" => "3",
    "follower" => "4",
    "mention" => "6",
    "comment" => "7",
    other => return Err(Error::input(format!("unknown notification type: {other}"))),
  })
}

/// One page of notices; the cursor is the oldest time of the last page.
pub async fn list(api: &Api, kind: Option<&str>, page: &PageReq) -> Result<Page<Notification>> {
  api.require_login()?;
  let params = [
    ("notice_type", notice_type(kind)?.to_owned()),
    ("count", page.size_within(PAGE_MAX).to_string()),
    (
      "max_time",
      page.cursor.clone().unwrap_or_else(|| "0".into()),
    ),
    ("min_time", "0".to_owned()),
  ];
  let v = api.get(NOTICE, &params).await?;
  // The notices come flat, or in groups (`notice_lists[].notice_list`).
  let group = v.at("notice_lists.0");
  let rows: Vec<&Value> = ["notice_list_v1", "noticeList", "notice_list"]
    .iter()
    .flat_map(|k| v.list(k))
    .chain(
      v.list("notice_lists")
        .iter()
        .flat_map(|g| g.list("notice_list")),
    )
    .collect();
  let items: Vec<Notification> = rows.into_iter().map(parse::notice).collect();
  let more = [&v, group]
    .iter()
    .any(|x| x.bool("has_more") == Some(true) || x.bool("hasMore") == Some(true));
  let next = [&v, group]
    .iter()
    .find_map(|x| x.str("min_time").filter(|t| t != "0"))
    .filter(|_| more && !items.is_empty());
  Ok(Page::new(items, next))
}
