//! Comments: those of a deal's or an article's page, page by page as its
//! pages show them (`<link>p<n>/`), in the order of the page's 最新
//! (newest) or 精彩 (hot) tab.

use std::sync::LazyLock;

use atp_core::{Comment, Page, PageReq, Result};
use regex::Regex;

use crate::api::Api;
use crate::{parse, refs};

/// `comments --sort`: the 最新 tab (newest first) or the 精彩 tab.
pub const SORTS: &[&str] = &["newest", "hot"];

static PAGER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"<a\b[^>]*pagination[^>]*>"#).unwrap());

pub async fn list(
  api: &Api,
  arg: &str,
  sort: Option<&str>,
  req: &PageReq,
) -> Result<Page<Comment>> {
  let target = refs::post(arg)?;
  let number = req.number_or(1);
  let mut url = target.url();
  if number > 1 {
    url.push_str(&format!("p{number}/"));
  }
  let html = api.page(&url).await?;
  let id = match sort {
    Some("hot") => "commentTabBlockHot",
    _ => "commentTabBlockNew",
  };
  let comments = parse::comments::list(parse::comments::block(&html, id));
  let wanted = format!("/p{}/", number + 1);
  let more = PAGER
    .find_iter(&html)
    .any(|tag| tag.as_str().contains(&wanted));
  let next = (more && !comments.is_empty()).then(|| (number + 1).to_string());
  Ok(Page::new(comments, next))
}
