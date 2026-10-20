//! Deals and articles: the front page's picks (`/homepage/json_more`,
//! twenty a page, paged by number and the time the list was cut at), the
//! search (`search.smzdm.com`, through the browser tab, twenty-odd rows a
//! page) and one deal or article read from its page.

use atp_core::text::encode_uri_component;
use atp_core::{Error, Page, PageReq, Post, Query, Result, ValueExt};
use jiff::Timestamp;

use crate::api::{Api, SEARCH};
use crate::{parse, refs};

/// The front page's picks (值得买精选), as its list loads them.
pub async fn hot(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let (number, cut) = match req.cursor.as_deref().and_then(|c| c.split_once(':')) {
    Some((number, cut)) => (number.parse().unwrap_or(1u64), cut.to_owned()),
    None => (1, Timestamp::now().as_second().to_string()),
  };
  let params = [("timesort", cut.clone()), ("p", number.to_string())];
  let v = api.json("/homepage/json_more", &params).await?;
  let items = v.list("data");
  let posts: Vec<Post> = items.iter().map(parse::item).collect();
  let next_cut = items
    .last()
    .and_then(|item| item.str("timesort"))
    .unwrap_or(cut);
  let next = (!posts.is_empty()).then(|| format!("{}:{next_cut}", number + 1));
  Ok(Page::new(posts, next))
}

/// `search WORDS`: the search's results across the site (综合), page by
/// page.
pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<Post>> {
  let words = query.keyword.trim();
  if words.is_empty() {
    return Err(Error::input("search for what? give some words"));
  }
  let number = req.number_or(1);
  let url = format!(
    "{SEARCH}/?c=home&s={}&v=b&p={number}",
    encode_uri_component(words)
  );
  let html = api.search_page(&url).await?;
  let posts = parse::rows::rows(&html);
  let more = html.contains(&format!("p={}", number + 1)) || posts.len() >= 20;
  let next = (more && !posts.is_empty()).then(|| (number + 1).to_string());
  Ok(Page::new(posts, next))
}

/// A deal or an article, read from its page.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let target = refs::post(arg)?;
  let url = target.url();
  let html = api.page(&url).await?;
  parse::page::page(&html, &url).ok_or_else(|| {
    Error::not_found(format!(
      "SMZDM shows no deal or article at {url}; it may have been taken down"
    ))
  })
}
