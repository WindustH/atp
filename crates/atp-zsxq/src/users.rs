//! People: the account (`/v3/users/self`), anyone's public profile
//! (`/v3/users/<uid>/public_info`, which the app also reads without a
//! login) and the topics a member posted in the groups you share with them
//! (their 足迹, `/v2/users/<uid>/footprints`, paged by `end_time`).

use atp_core::{Error, Page, PageReq, Post, Result, User, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

pub async fn whoami(api: &Api) -> Result<User> {
  api.need_login()?;
  let v = api.get("/v3/users/self", &[]).await?;
  let mut user = parse::user(v.at("user")).ok_or_else(|| {
    Error::auth("知识星球 shows no account for this session").with_hint(api.ctx.login_hint())
  })?;
  user.raw = Some(v);
  Ok(user)
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let uid = refs::user(arg)?;
  let v = api
    .get(&format!("/v3/users/{uid}/public_info"), &[])
    .await?;
  let info = v.at("user_public_info");
  let mut user = ["user", ""]
    .iter()
    .find_map(|path| parse::user(info.at(path)))
    .ok_or_else(|| Error::not_found(format!("知识星球 shows no user {uid}")))?;
  if user.id.is_empty() {
    user.id = uid.clone();
    user.url = Some(parse::user_url(&uid));
  }
  user.raw = Some(v);
  Ok(user)
}

/// The topics of a member's 足迹, newest first; notes (笔记) are left out.
pub async fn posts(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let uid = refs::user(arg)?;
  api.need_login()?;
  let count = req.size_within(20);
  let mut params = vec![("count", count.to_string())];
  if let Some(end) = &req.cursor {
    params.push(("end_time", end.clone()));
  }
  let v = api
    .get(&format!("/v2/users/{uid}/footprints"), &params)
    .await?;
  let items = v.list("footprints");
  let posts = items
    .iter()
    .filter(|item| item.str("type").as_deref() == Some("topic"))
    .filter_map(|item| parse::topic(item.at("topic")))
    .collect();
  let next = if items.len() < count {
    None
  } else {
    items
      .last()
      .and_then(|item| item.first_str(&["topic.create_time", "note.create_time"]))
      .and_then(|t| parse::shifted(&t, -1))
  };
  Ok(Page::new(posts, next))
}
