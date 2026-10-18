//! Comment threads: an article's whole discussion in one request
//! (`/api/comments?a_id=`), threads in the page's order with every reply
//! inside; one comment with its replies (`/api/comments/<id code>`).

use atp_core::paging::slice;
use atp_core::{Comment, Error, Page, PageReq, Result, ValueExt};

use crate::api::Api;
use crate::refs::{self, ArticleRef};
use crate::{articles, parse};

pub async fn comments(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Comment>> {
  let id = article_id(api, arg).await?;
  Ok(slice(thread(api, id).await?, page))
}

/// The replies under one comment.
pub async fn replies(api: &Api, comment: &str, page: &PageReq) -> Result<Page<Comment>> {
  let code = refs::comment(comment)?;
  let v = api.get(&format!("comments/{code}"), Vec::new()).await?;
  let comment = parse::comment(&v, &api.instance());
  Ok(slice(comment.replies, page))
}

/// The article's numeric id (links are looked up).
async fn article_id(api: &Api, arg: &str) -> Result<u64> {
  match refs::article(arg)? {
    ArticleRef::Id(id) => Ok(id),
    path => articles::json(api, &path)
      .await?
      .u64("id")
      .ok_or_else(|| Error::upstream(format!("{} sent an article without an id", api.host()))),
  }
}

/// The threads of an article, loaded once per run.
async fn thread(api: &Api, id: u64) -> Result<Vec<Comment>> {
  if let Some(thread) = api.threads.borrow().get(&id) {
    return Ok(thread.clone());
  }
  let v = api.get("comments", vec![("a_id", id.to_string())]).await?;
  let instance = api.instance();
  let thread: Vec<Comment> = v
    .list("")
    .iter()
    .map(|c| parse::comment(c, &instance))
    .collect();
  api.threads.borrow_mut().insert(id, thread.clone());
  Ok(thread)
}
