//! Threads: the hot threads of the front page or of one zone
//! (`window.$$data.pageData.threads` of `/` and `/all-<zone>`), and one
//! thread read from its page (`__NEXT_DATA__`'s `detail.thread`).

use atp_core::{Error, Page, PageReq, Post, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// `hot --category`: the zones of the site, by the name of their page
/// (`gambia` is 步行街, `/all-gambia`).
pub const HOT_CATEGORIES: &[&str] = &[
  "gambia", "nba", "soccer", "csl", "cba", "sports", "lol", "gg", "ent", "gear", "digital", "cars",
];

/// The hot threads of the front page, or of a zone. One page holds them all.
pub async fn hot(api: &Api, zone: Option<&str>, _req: &PageReq) -> Result<Page<Post>> {
  let path = match zone {
    Some(zone) => format!("/all-{zone}"),
    None => "/".to_owned(),
  };
  let v = api.list_data(&path).await?;
  let list = v.list("pageData.threads");
  if list.is_empty() && v.at("pageData").is_null() {
    return Err(Error::upstream(format!(
      "Hupu's page {path} lists no threads; the site may have changed"
    )));
  }
  Ok(Page::last(list.iter().map(parse::thread).collect()))
}

/// The page of a thread (`page` > 1 for later pages of its replies): its
/// `detail`, or the error the page reports.
pub async fn detail(api: &Api, tid: &str, page: u64) -> Result<Value> {
  let path = if page > 1 {
    format!("/{tid}-{page}.html")
  } else {
    format!("/{tid}.html")
  };
  let props = api.thread_data(&path).await?;
  let code = props.i64("detailErrorInfo.code").unwrap_or(200);
  let detail = props.at("detail");
  if code != 200 || detail.at("thread").is_null() {
    let why = props
      .str("detailErrorInfo.message")
      .unwrap_or_else(|| "no such thread".into());
    return Err(Error::not_found(format!("Hupu thread {tid}: {why}")));
  }
  Ok(detail.clone())
}

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let tid = refs::thread(arg)?;
  let detail = detail(api, &tid, 1).await?;
  Ok(parse::thread(detail.at("thread")))
}
