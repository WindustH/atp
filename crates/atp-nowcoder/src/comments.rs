//! Comments: a post's comments and a comment's replies
//! (`/api/sparta/comment/list-by-page`, by heat or by time, 20 a page) and
//! commenting or replying (`/comment/create`, the form the post page sends).

use atp_core::{Action, Comment, Error, Page, PageReq, Reply, Result, Value, ValueExt, json};

use crate::api::{Api, Body, Route};
use crate::{parse, posts, refs};

/// `comments --sort`: by heat (the default, as the post page opens) or
/// newest first.
pub const SORTS: &[&str] = &["hot", "new"];
/// The entity type of comments, for their replies.
const COMMENT: i64 = 2;
/// Pages of comments searched for the one `--reply-to` names.
const REPLY_PAGES: u64 = 5;

/// `order`: 0 newest, 1 the site's default, 2 by heat.
fn order(sort: Option<&str>) -> u8 {
  match sort {
    Some("new") => 0,
    _ => 2,
  }
}

/// One page of the comments on an entity (a post, or a comment for its
/// replies).
async fn thread(api: &Api, id: &str, kind: i64, order: u8, page: u64) -> Result<Value> {
  let params = [
    ("entityId", id.to_owned()),
    ("entityType", kind.to_string()),
    ("order", order.to_string()),
    ("pageNo", page.to_string()),
    ("toCommentId", "0".to_owned()),
  ];
  api
    .get(Route::Gateway, "/api/sparta/comment/list-by-page", &params)
    .await
}

fn comments_page(v: &Value, page: u64) -> Page<Comment> {
  let items: Vec<Comment> = v.list("records").iter().map(parse::comment).collect();
  let current = v.u64("current").unwrap_or(page);
  let more = v.u64("totalPage").is_some_and(|pages| current < pages);
  let cursor = (more && !items.is_empty()).then(|| (current + 1).to_string());
  Page::new(items, cursor)
}

pub async fn list(
  api: &Api,
  arg: &str,
  sort: Option<&str>,
  req: &PageReq,
) -> Result<Page<Comment>> {
  let target = posts::target(api, arg, false).await?;
  let page = req.number_or(1);
  let v = thread(api, &target.id, target.kind, order(sort), page).await?;
  Ok(comments_page(&v, page))
}

/// The replies to one comment, oldest first as the site lists them.
pub async fn replies(api: &Api, comment: &str, req: &PageReq) -> Result<Page<Comment>> {
  let id = refs::comment(comment)?;
  let page = req.number_or(1);
  let v = thread(api, &id, COMMENT, 1, page).await?;
  Ok(comments_page(&v, page))
}

/// The author of a comment of the post, found among its first pages.
async fn comment_author(api: &Api, target: &posts::Target, comment: &str) -> Result<String> {
  for page in 1..=REPLY_PAGES {
    let v = thread(api, &target.id, target.kind, order(None), page).await?;
    let found = v
      .list("records")
      .iter()
      .find(|c| c.str("id").as_deref() == Some(comment));
    if let Some(author) = found.and_then(|c| c.first_str(&["authorId", "userBrief.userId"])) {
      return Ok(author);
    }
    if v.u64("totalPage").is_none_or(|pages| page >= pages) {
      break;
    }
  }
  Err(Error::not_found(format!(
    "comment {comment} is not among the first {REPLY_PAGES} pages of the post's comments; --reply-to takes a comment as `comments` lists it, not a reply"
  )))
}

/// Comment on a post, or answer one of its comments (`--reply-to`).
pub async fn add(api: &Api, arg: &str, reply: &Reply) -> Result<Action> {
  let text = reply.text.trim();
  if text.is_empty() {
    return Err(Error::input("the comment is empty"));
  }
  if !reply.images.is_empty() {
    return Err(Error::unsupported("comment images on Nowcoder"));
  }
  api.require_login()?;
  let target = posts::target(api, arg, true).await?;
  let content = json!({"pureText": text, "imgs": []}).to_string();
  let mut form = vec![
    ("commentContent", text.to_owned()),
    ("contentV2", content),
    ("toFeed", "false".to_owned()),
    ("isAnonymousFlag", "false".to_owned()),
  ];
  match &reply.reply_to {
    None => {
      let author = target.author.clone().unwrap_or_default();
      form.extend([
        ("entityType", target.kind.to_string()),
        ("entityId", target.id.clone()),
        ("entityOwnerId", author.clone()),
        ("toUserId", author),
      ]);
    }
    Some(comment) => {
      let comment = refs::comment(comment)?;
      let author = comment_author(api, &target, &comment).await?;
      form.extend([
        ("entityType", COMMENT.to_string()),
        ("entityId", comment.clone()),
        ("entityOwnerId", author.clone()),
        ("toUserId", author),
        ("toCommentId", comment),
      ]);
    }
  }
  let v = api
    .write(Route::Site, "/comment/create", &[], Body::Form(&form))
    .await?;
  let id = target.url.rsplit('/').next().unwrap_or_default().to_owned();
  let action = Action::done("comment", &id).with_url(target.url);
  let new = v
    .first_str(&["id", "commentId", "comment.id"])
    .or_else(|| v.as_i64().map(|n| n.to_string()));
  Ok(match new {
    Some(new) => action.with_id(new),
    None => action,
  })
}
