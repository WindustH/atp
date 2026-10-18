//! Comments: a post's comments with their nested replies
//! (`/posts/<id>/comments`, by heat or by time) and commenting or replying
//! (`POST /posts/<id>/comments`).

use atp_core::http::Method;
use atp_core::{Action, Comment, Error, Page, PageReq, Reply, Result, ValueExt, json};

use crate::api::Api;
use crate::{parse, refs};

/// `comments --sort`: by heat (the default), newest first, oldest first.
pub const SORTS: &[&str] = &["hot", "new", "old"];

fn order(sort: Option<&str>) -> &'static str {
  match sort {
    Some("new") => "-published_at",
    Some("old") => "published_at",
    _ => "-hot",
  }
}

pub async fn list(
  api: &Api,
  arg: &str,
  sort: Option<&str>,
  req: &PageReq,
) -> Result<Page<Comment>> {
  let id = refs::post(arg)?;
  let params = [
    ("page", req.number_or(1).to_string()),
    ("sort", order(sort).to_owned()),
  ];
  let v = api.get(&format!("/posts/{id}/comments"), &params).await?;
  let items = v.list("comments").iter().map(parse::comment).collect();
  Ok(Page::new(items, parse::next_page(&v)))
}

/// The HTML the site's editor renders next to the Markdown: paragraphs,
/// line breaks, the text escaped.
fn html(text: &str) -> String {
  text
    .split("\n\n")
    .map(str::trim)
    .filter(|p| !p.is_empty())
    .map(|p| {
      let escaped = p
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', "<br>");
      format!("<p>{escaped}</p>")
    })
    .collect()
}

/// Comment on a post, or answer one of its comments (`--reply-to`).
pub async fn add(api: &Api, arg: &str, reply: &Reply) -> Result<Action> {
  let id = refs::post(arg)?;
  let text = reply.text.trim();
  if text.is_empty() {
    return Err(Error::input("the comment is empty"));
  }
  let mut body = json!({
    "content": html(text),
    "raw_content": text,
  });
  if let Some(parent) = &reply.reply_to {
    body["parent_id"] = json!(refs::comment(parent)?);
  }
  let v = api
    .write(Method::POST, &format!("/posts/{id}/comments"), Some(body))
    .await?;
  let action = Action::done("comment", &id);
  Ok(match v.str("comment.id").or_else(|| v.str("id")) {
    Some(new) => action
      .with_url(format!("{}#comment-{new}", parse::post_url(&id)))
      .with_id(new),
    None => action.with_url(parse::post_url(&id)),
  })
}
