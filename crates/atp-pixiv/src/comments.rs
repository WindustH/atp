//! The comments of a work as its page loads them: the comments themselves
//! (`comments/roots`, newest first, by offset), and the replies under one
//! of them (`comments/replies`, by page). Artworks and novels have the
//! same calls under `illusts` and `novels`.

use atp_core::{Comment, Error, Page, PageReq, Result, ValueExt};

use crate::api::{Api, Query};
use crate::parse;
use crate::refs::{self, WorkRef};

/// The most comments a page asks for.
const PAGE: usize = 50;

/// `illusts` or `novels`, and the parameter naming the work.
fn route(work: &WorkRef) -> (&'static str, &'static str) {
  match work {
    WorkRef::Artwork(_) => ("illusts", "illust_id"),
    WorkRef::Novel(_) => ("novels", "novel_id"),
  }
}

pub async fn list(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Comment>> {
  let work = refs::work(arg)?;
  let (kind, param) = route(&work);
  let offset = page.number_or(0);
  let size = page.size_within(PAGE);
  let query: Query = vec![
    (param, work.id().to_owned()),
    ("offset", offset.to_string()),
    ("limit", size.to_string()),
  ];
  let v = api
    .get(&format!("/ajax/{kind}/comments/roots"), query)
    .await?;
  let comments: Vec<Comment> = v.list("comments").iter().map(parse::comment).collect();
  let next = (v.bool("hasNext") == Some(true) && !comments.is_empty())
    .then(|| (offset + comments.len() as u64).to_string());
  Ok(Page::new(comments, next))
}

pub async fn replies(api: &Api, arg: &str, comment: &str, page: &PageReq) -> Result<Page<Comment>> {
  let work = refs::work(arg)?;
  let (kind, _) = route(&work);
  let comment = comment.trim();
  if comment.is_empty() || !comment.bytes().all(|b| b.is_ascii_digit()) {
    return Err(Error::input(format!(
      "not a pixiv comment: {comment} (expected the id `comments` prints)"
    )));
  }
  let no = page.number_or(1);
  let query: Query = vec![("comment_id", comment.to_owned()), ("page", no.to_string())];
  let v = api
    .get(&format!("/ajax/{kind}/comments/replies"), query)
    .await?;
  let replies: Vec<Comment> = v.list("comments").iter().map(parse::comment).collect();
  let next = (v.bool("hasNext") == Some(true) && !replies.is_empty()).then(|| (no + 1).to_string());
  Ok(Page::new(replies, next))
}
