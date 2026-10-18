//! Replies (回帖): a thread's replies with the first few answers to each
//! nested under them (`/api/threads/<tid>/nested-posts`), and with a
//! login all the answers to one reply (`/api/posts/<pid>/nested-posts`,
//! which the API refuses guests) and replying to a thread
//! (`POST /api/threads/<tid>/posts`, as the site's pages send it).

use atp_core::{Action, Comment, Error, Page, PageReq, Reply, Result, ValueExt, json};

use crate::api::Api;
use crate::{parse, refs};

/// `comments --sort`: oldest first (the thread's order), newest first,
/// hottest first.
pub const SORTS: &[&str] = &["oldest", "newest", "hot"];
/// Replies a page, as the site's pages ask for them.
const PER_PAGE: usize = 20;
/// Answers carried inline under each reply.
const INLINE: u32 = 5;

fn order(sort: Option<&str>) -> &'static str {
  match sort {
    Some("newest") => "time_desc",
    Some("hot") => "hot_desc",
    _ => "time_asc",
  }
}

/// One page of `posts`, the next while pages come full.
async fn page(
  api: &Api,
  path: &str,
  mut params: Vec<(&str, String)>,
  req: &PageReq,
) -> Result<Page<Comment>> {
  let number = req.number_or(1);
  params.push(("ps", PER_PAGE.to_string()));
  params.push(("pg", number.to_string()));
  let v = api.get(path, &params).await?;
  let comments: Vec<Comment> = v.list("posts").iter().map(parse::comment).collect();
  let more = comments.len() >= PER_PAGE;
  Ok(Page::new(comments, more.then(|| (number + 1).to_string())))
}

pub async fn list(
  api: &Api,
  arg: &str,
  sort: Option<&str>,
  req: &PageReq,
) -> Result<Page<Comment>> {
  let tid = refs::thread(arg)?;
  let params = vec![
    ("order", order(sort).to_owned()),
    ("sub_limit", INLINE.to_string()),
  ];
  page(
    api,
    &format!("/api/threads/{tid}/nested-posts"),
    params,
    req,
  )
  .await
}

/// Every answer to one reply (needs a login).
pub async fn replies(api: &Api, comment: &str, req: &PageReq) -> Result<Page<Comment>> {
  let pid = refs::post(comment)?;
  page(
    api,
    &format!("/api/posts/{pid}/nested-posts"),
    Vec::new(),
    req,
  )
  .await
}

/// Reply to a thread, under the account's name (not anonymously).
pub async fn add(api: &Api, arg: &str, reply: &Reply) -> Result<Action> {
  let tid = refs::thread(arg)?;
  if reply.reply_to.is_some() {
    return Err(Error::input(
      "ATP replies to the thread itself on 1Point3Acres; --reply-to is not supported",
    ));
  }
  let text = reply.text.trim();
  if text.is_empty() {
    return Err(Error::input("the reply is empty"));
  }
  let body = json!({"message": text, "anonymous": 0});
  let v = api
    .post(&format!("/api/threads/{tid}/posts"), &body)
    .await?;
  let tid = tid.to_string();
  let action = Action::done("comment", &tid).with_url(parse::thread_url(&tid));
  Ok(match v.str("data.pid") {
    Some(pid) => action.with_id(pid),
    None => action,
  })
}
