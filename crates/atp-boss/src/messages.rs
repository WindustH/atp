//! Your chats with recruiters, through the requests the job seeker's chat
//! page (`/web/geek/chat`) makes:
//!
//! - `zprelation/friend/geekFilterByLabel` (`labelId` 0: every chat):
//!   the chats, all at once;
//! - `zprelation/friend/getGeekFriendList.json` (POST `friendIds`): their
//!   details, among them the `securityId` a chat's messages need;
//! - `zpchat/geek/historyMsg` (`bossId`, `securityId`, `page`, 20 a page):
//!   a chat's messages.
//!
//! Sending is left out: the chat page sends over MQTT with protobuf
//! frames. So are exchanging phone numbers or WeChat ids and sending
//! résumés.

use atp_core::{Collection, Error, Page, PageReq, Post, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// The chat page the requests come from.
const CHAT: &str = "https://www.zhipin.com/web/geek/chat";
/// Chats per page, and details asked for at once.
const PAGE: usize = 20;
/// Messages per page, as the chat page asks for them.
const MESSAGES: usize = 20;

/// The details of the chats `ids`, by `friendId` (or `uid`).
async fn details(api: &Api, ids: &[String]) -> Result<Vec<Value>> {
  if ids.is_empty() {
    return Ok(Vec::new());
  }
  let form = [("friendIds", ids.join(","))];
  let v = api
    .post(
      "/wapi/zprelation/friend/getGeekFriendList.json",
      &[],
      &form,
      CHAT,
    )
    .await?;
  Ok(v.list("result").to_vec())
}

/// The id a chat goes by.
fn friend(v: &Value) -> Option<String> {
  v.str("friendId").or_else(|| v.str("uid"))
}

pub async fn chats(api: &Api, req: &PageReq) -> Result<Page<Collection>> {
  api.require_login()?;
  let query = [
    ("labelId", "0".to_owned()),
    ("encryptSystemId", String::new()),
  ];
  let v = api
    .get("/wapi/zprelation/friend/geekFilterByLabel", &query, CHAT)
    .await?;
  let all = v.list("friendList").to_vec();
  let page = atp_core::paging::slice(
    all,
    &PageReq {
      cursor: req.cursor.clone(),
      size: PAGE,
    },
  );
  let ids: Vec<String> = page.items.iter().filter_map(friend).collect();
  let details = details(api, &ids).await?;
  let items = page
    .items
    .iter()
    .filter_map(|brief| {
      let id = friend(brief)?;
      let mut merged = brief.clone();
      if let (Some(map), Some(more)) = (
        merged.as_object_mut(),
        details
          .iter()
          .find(|d| friend(d).as_deref() == Some(id.as_str())),
      ) {
        for (key, value) in more.as_object().into_iter().flatten() {
          map.insert(key.clone(), value.clone());
        }
      }
      parse::chat(&merged)
    })
    .collect();
  Ok(Page::new(items, page.next_cursor))
}

pub async fn messages(api: &Api, chat: &str, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let id = refs::chat(chat)?;
  let found = details(api, std::slice::from_ref(&id)).await?;
  let detail = found.first().ok_or_else(|| {
    Error::not_found(format!(
      "no chat {id} among yours (`atp boss chats` lists them)"
    ))
  })?;
  let other = detail.str("uid").unwrap_or_else(|| id.clone());
  let security = detail
    .str("securityId")
    .ok_or_else(|| Error::upstream("BOSS Zhipin named no securityId for this chat's messages"))?;
  let page = req.number_or(1);
  let query = [
    ("bossId", other.clone()),
    ("securityId", security),
    ("page", page.to_string()),
    ("c", MESSAGES.to_string()),
    ("src", "0".to_owned()),
  ];
  let v = api
    .get("/wapi/zpchat/geek/historyMsg", &query, CHAT)
    .await?;
  let list = match v.get("messages") {
    Some(_) => v.list("messages"),
    None => v.list("historyMsgList"),
  };
  let items: Vec<Post> = list
    .iter()
    .map(|m| parse::message(m, &id, &other))
    .collect();
  let more = v.bool("hasMore").unwrap_or(items.len() >= MESSAGES);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}
