//! Talks (宣讲会): the talk list of the older site (25 a page, from today
//! on, in the site's order), narrowed by words, city, province, school,
//! company, day and industry, and a talk's page completed by the mobile
//! site's (times, and the city and description when missing).

use atp_core::{Error, ErrorCode, Page, PageReq, Post, Result};

use crate::api::{Api, MY, gbk_query};
use crate::{parse, refs};

/// What the list is narrowed to, in the site's codes.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  /// Words in a company's or school's name or the venue.
  pub words: String,
  pub city: Option<String>,
  pub province: Option<String>,
  pub school: Option<String>,
  pub company: Option<String>,
  /// `2026-10-22`: that day only.
  pub date: Option<String>,
  pub industry: Option<String>,
}

/// The list's page as its pager links it.
fn list_url(filter: &Filter, page: u64) -> String {
  let or = |value: &Option<String>, empty: &str| value.clone().unwrap_or_else(|| empty.to_owned());
  format!(
    "{MY}/index.php/personal/xjhinfo.htm/?page={page}&cid={}&city={}&word={}&province={}&schoolid={}&sdate={}&hyid={}",
    or(&filter.company, ""),
    or(&filter.city, "0"),
    gbk_query(&filter.words),
    or(&filter.province, "0"),
    or(&filter.school, ""),
    or(&filter.date, ""),
    or(&filter.industry, "0"),
  )
}

/// One page of the list.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let html = api.page(&list_url(filter, page)).await?;
  let items = parse::rows(&html);
  let last = parse::last_page(&html).unwrap_or(page);
  let more = !items.is_empty() && page < last;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// A talk's page, with its start and end times (and the city and
/// description when the page has none) from the mobile site's page when
/// that answers.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::talk(arg)?;
  let url = refs::talk_url(&id);
  let gone = || {
    Error::not_found(format!(
      "Yingjiesheng shows no talk at {url} (it may have been taken down)"
    ))
  };
  let html = match api.page(&url).await {
    Err(e) if e.code == ErrorCode::NotFound => return Err(gone()),
    other => other?,
  };
  let mut post = parse::talk(&html, &id).ok_or_else(gone)?;
  match api.mobile(&refs::mobile_url(&id)).await {
    Ok(page) => parse::fill(&page, &mut post),
    Err(e) => tracing::debug!("the mobile page of talk {id} did not answer: {e}"),
  }
  Ok(post)
}
