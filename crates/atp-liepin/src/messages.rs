//! Your chats with recruiters (聊一聊), through the requests the site's
//! chat window makes. Each carries the job seeker's IM identity the login
//! leaves in cookies: `imUserType` 0, `imId` (cookie `imId_0`) and
//! `imApp` (cookie `imApp_0`, else 1).
//!
//! - `im.c.contact.get-contact-list`: the chats, paged by the last
//!   chat's `sortValue`;
//! - `im.c.chat.chat-list`: a chat's messages, newest first, paged by the
//!   oldest `msgId` (`-1` for the newest);
//! - `im.c.contact.get-single-user-info`: who is on the other side of a
//!   chat;
//! - `im.c.chat.send-common-msg`: a text message, its body the JSON
//!   `{ext, bodies: [{msg, type: "txt"}], push}` the window builds.
//!
//! Opening new chats (which sends a greeting), exchanging phone numbers
//! or WeChat ids and sending résumés are left out.

use std::collections::BTreeMap;

use atp_core::chat::message_id;
use atp_core::{Action, Collection, Error, Message, Page, PageReq, Post, Result, ValueExt, json};

use crate::api::{Api, WWW};
use crate::{parse, refs};

/// The job seeker's side of the IM.
const SEEKER: &str = "0";
/// Messages per page, as the chat window asks for them.
const PAGE: usize = 20;

/// The IM identity every chat request carries.
fn identity(api: &Api) -> Result<Vec<(&'static str, String)>> {
  api.require_login()?;
  let id = api.cookie(&format!("imId_{SEEKER}")).ok_or_else(|| {
    Error::auth("the session has no chat identity (cookie imId_0)").with_hint(api.ctx.login_hint())
  })?;
  let app = api
    .cookie(&format!("imApp_{SEEKER}"))
    .unwrap_or_else(|| "1".into());
  Ok(vec![
    ("imUserType", SEEKER.to_owned()),
    ("imId", id),
    ("imApp", app),
  ])
}

/// `page:sortValue`: the page number and the last chat's sort value.
fn chat_cursor(req: &PageReq) -> (u64, String) {
  match req.cursor.as_deref().and_then(|c| c.split_once(':')) {
    Some((page, sort)) => (page.parse().unwrap_or(0), sort.to_owned()),
    None => (0, String::new()),
  }
}

async fn contacts(api: &Api, page: u64, sort: &str, size: usize) -> Result<Page<Collection>> {
  let mut form = identity(api)?;
  form.extend([
    ("sortValue", sort.to_owned()),
    ("pageSize", size.to_string()),
    ("curPage", page.to_string()),
  ]);
  let v = api.form("im.c.contact.get-contact-list", &form).await?;
  let rows = v.list("list");
  let items: Vec<Collection> = rows.iter().map(parse::chat).collect();
  let next = (v.bool("hasMore") == Some(true))
    .then(|| rows.last().and_then(|c| c.str("sortValue")))
    .flatten()
    .map(|sort| format!("{}:{sort}", page + 1));
  Ok(Page::new(items, next))
}

pub async fn chats(api: &Api, req: &PageReq) -> Result<Page<Collection>> {
  let (page, sort) = chat_cursor(req);
  contacts(api, page, &sort, PAGE).await
}

pub async fn messages(api: &Api, chat: &str, req: &PageReq) -> Result<Page<Post>> {
  let chat = refs::chat(chat)?;
  let mut form = identity(api)?;
  form.extend([
    (
      "maxMessageId",
      req.cursor.clone().unwrap_or_else(|| "-1".into()),
    ),
    ("oppositeImId", chat.clone()),
    ("pageSize", PAGE.to_string()),
  ]);
  let v = api.form("im.c.chat.chat-list", &form).await?;
  let rows = v.list("list");
  let items: Vec<Post> = rows.iter().map(|m| parse::message(m, &chat)).collect();
  let next = (rows.len() >= PAGE)
    .then(|| rows.last().and_then(|m| m.str("msgId")))
    .flatten();
  Ok(Page::new(items, next))
}

/// Who is on the other side of one of your chats: their user id and IM
/// user type (2 for recruiters).
async fn other_side(api: &Api, chat: &str) -> Result<(String, String)> {
  let form = [("oppositeImId", chat.to_owned())];
  let v = api.form("im.c.contact.get-single-user-info", &form).await?;
  let user = v
    .str("oppositeUserId")
    .ok_or_else(|| Error::not_found(format!("no chat {chat} among yours (`chats` lists them)")))?;
  let kind = v.str("oppositeImUserType").unwrap_or_else(|| "2".into());
  Ok((user, kind))
}

/// A text message to one of your chats.
pub async fn send(api: &Api, chat: &str, message: &Message) -> Result<Action> {
  if !message.files.is_empty() || message.reply_to.is_some() {
    return Err(Error::unsupported(
      "send --attach / --reply-to (Liepin sends text only)",
    ));
  }
  let chat = refs::chat(chat)?;
  let mut form = identity(api)?;
  let client = api.cookie(&format!("imClientId_{SEEKER}")).ok_or_else(|| {
    Error::auth("the session has no chat client id (cookie imClientId_0)")
      .with_hint(api.ctx.login_hint())
  })?;
  let (user, kind) = other_side(api, &chat).await?;
  let payload = json!({
    "ext": {"extType": 1, "extBody": {"bizType": "1", "bsData": {}}},
    "bodies": [{"msg": message.text, "type": "txt"}],
    "push": "1",
  });
  form.extend([
    ("save", String::new()),
    ("count", "1".to_owned()),
    ("imClientId", client),
    ("oppositeImId", chat.clone()),
    ("oppositeUserId", user),
    ("oppositeImUserType", kind),
    ("chatType", "0".to_owned()),
    (
      "msgTime",
      jiff::Timestamp::now().as_millisecond().to_string(),
    ),
    ("msgType", "txt".to_owned()),
    ("payload", payload.to_string()),
  ]);
  let v = api
    .write("im.c.chat.send-common-msg", &form, &format!("{WWW}/"))
    .await?;
  let mut action = Action::done("send", &chat);
  if let Some(id) = v.str("msgId") {
    action = action.with_id(message_id(&chat, &id));
  }
  Ok(action)
}

/// Unread messages: the sum over your 50 most recent chats.
pub async fn unread(api: &Api) -> Result<BTreeMap<String, u64>> {
  let page = contacts(api, 0, "", 50).await?;
  let total = page
    .items
    .iter()
    .filter_map(|c| c.extra.get("unread").and_then(|n| n.as_u64()))
    .sum();
  Ok(BTreeMap::from([("messages".to_owned(), total)]))
}
