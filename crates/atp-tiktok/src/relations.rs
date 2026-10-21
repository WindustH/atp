//! Your relations beyond following: the accounts you block, as the
//! settings' "Blocked accounts" page lists them (`/api/block/user_list/`),
//! and approving requests to follow your private account, as the inbox
//! does (`/api/commit/follow/request/approve/`).

use atp_core::bulk;
use atp_core::{Action, Error, Page, PageReq, Result, User, ValueExt};

use crate::api::Api;
use crate::{parse, users};

const BLOCKED: &str = "/api/block/user_list/";
const APPROVE: &str = "/api/commit/follow/request/approve/";
/// The page asks for 20 at a time.
const BLOCKED_PAGE: usize = 20;
/// `from` of the inbox, where the web approves requests.
const FROM_INBOX: &str = "33";

/// The accounts you block, paged by `offset`.
pub async fn blocked(api: &Api, req: &PageReq) -> Result<Page<User>> {
  api.require_login()?;
  let params = vec![
    ("offset", req.cursor.clone().unwrap_or_else(|| "0".into())),
    ("count", req.size_within(BLOCKED_PAGE).to_string()),
  ];
  let v = api.get(BLOCKED, &params).await?;
  let items: Vec<User> = v.list("userList").iter().map(parse::listed_user).collect();
  let more = v.bool("hasMore").unwrap_or(false) && !items.is_empty();
  let next = more.then(|| v.str("offset")).flatten();
  Ok(Page::new(items, next))
}

/// Let users who asked to follow your private account follow it, one
/// call each (each looked up for its user id first).
pub async fn approve(api: &Api, inputs: &[String]) -> Result<Vec<Action>> {
  api.require_login()?;
  bulk::each("approve-follower", inputs, async |input| {
    let (_, id) = users::ids(api, Some(input)).await?;
    if id.is_empty() {
      return Err(Error::upstream("the profile carries no user id"));
    }
    api.write_pause().await;
    let params = [("from_user_id", id.clone()), ("from", FROM_INBOX.into())];
    api.post(APPROVE, &params).await?;
    Ok(Action::done("approve-follower", input.trim()).with_id(id))
  })
  .await
}
