//! A topic's comments, oldest first as the app shows them
//! (`/v2/topics/<tid>/comments?sort=asc&with_sticky=true`), paged by
//! `begin_time`, a millisecond after the last comment. The first page
//! starts with the pinned comment. Replies come nested in their comment
//! (`replied_comments`), as many as the API puts there.

use atp_core::{Comment, Page, PageReq, Result, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// The page size of the app.
const PAGE: usize = 30;

pub async fn list(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Comment>> {
  let tid = refs::topic(arg)?;
  api.need_login()?;
  let count = req.size_within(PAGE);
  let mut params = vec![
    ("sort", "asc".to_owned()),
    ("count", count.to_string()),
    ("with_sticky", "true".to_owned()),
  ];
  if let Some(begin) = &req.cursor {
    params.push(("begin_time", begin.clone()));
  }
  let v = api
    .get(&format!("/v2/topics/{tid}/comments"), &params)
    .await?;
  let listed = v.list("comments");
  let mut comments: Vec<Comment> = Vec::new();
  if req.cursor.is_none() {
    comments.extend(v.list("sticky_comments").iter().filter_map(parse::comment));
  }
  for comment in listed.iter().filter_map(parse::comment) {
    if !comments.iter().any(|c| c.id == comment.id) {
      comments.push(comment);
    }
  }
  let next = if listed.len() < count {
    None
  } else {
    listed
      .last()
      .and_then(|c| c.str("create_time"))
      .and_then(|t| parse::shifted(&t, 1))
  };
  Ok(Page::new(comments, next))
}
