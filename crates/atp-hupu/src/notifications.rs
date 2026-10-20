//! Mentions (提到我的): the replies that mention the account, as its
//! message page lists them (`getMentionedRemindList`, `plate` 1), paged
//! by the `pageStr` the site returns.

use atp_core::{Notification, Page, PageReq, Result, ValueExt};

use crate::api::Api;
use crate::parse;

/// The mentions page the request comes from.
const REFERER: &str = "https://my.hupu.com/message?tabKey=1";

pub async fn mentions(api: &Api, req: &PageReq) -> Result<Page<Notification>> {
  let mut params = vec![("plate", "1".to_owned())];
  if let Some(cursor) = req.cursor.as_deref().filter(|c| !c.is_empty()) {
    params.push(("pageStr", cursor.to_owned()));
  }
  let v = api
    .my(
      "/pcmapi/pc/space/v1/getMentionedRemindList",
      &params,
      REFERER,
    )
    .await?;
  let data = v.at("data");
  let items = data.list("hisList").iter().map(parse::mention).collect();
  let more = data.bool("hasNextPage") == Some(true);
  let next = data
    .str("pageStr")
    .filter(|next| more && Some(next.as_str()) != req.cursor.as_deref());
  Ok(Page::new(items, next))
}
