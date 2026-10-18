//! Articles: the popular, top, rising, fresh and latest lists, a tag's
//! and a user's articles, single articles with their Markdown, your
//! drafts, and the tags.

use atp_core::{Collection, Page, PageReq, Post, Result, Value};

use crate::api::{Api, Query};
use crate::refs::{self, ArticleRef};
use crate::{parse, users};

/// `hot --category`: Forem's popular list, rising articles, or the most
/// liked of a period.
pub const HOT_CATEGORIES: &[&str] = &[
  "popular", "rising", "day", "week", "month", "year", "infinity",
];
/// `feed --type`: newest first, or new articles with few reactions yet.
pub const FEED_KINDS: &[&str] = &["latest", "fresh"];
/// The periods of `top` (`tag --top`).
pub const PERIODS: &[&str] = &["day", "week", "month", "year", "infinity"];

/// A period in days, as `top` takes it.
fn days(period: &str) -> Option<u32> {
  match period {
    "day" => Some(1),
    "week" => Some(7),
    "month" => Some(30),
    "year" => Some(365),
    "infinity" => Some(36_500),
    _ => None,
  }
}

async fn listing(api: &Api, path: &str, query: Query, page: &PageReq) -> Result<Page<Post>> {
  let (rows, next) = api.list(path, query, page).await?;
  let instance = api.instance();
  let posts = rows
    .iter()
    .map(|row| parse::article(row, &instance))
    .collect();
  Ok(Page::new(posts, next))
}

pub async fn hot(api: &Api, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  let query = match category.unwrap_or(HOT_CATEGORIES[0]) {
    "popular" => Vec::new(),
    "rising" => vec![("state", "rising".to_owned())],
    period => days(period)
      .map(|d| vec![("top", d.to_string())])
      .unwrap_or_default(),
  };
  listing(api, "articles", query, page).await
}

pub async fn feed(api: &Api, kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  match kind.unwrap_or(FEED_KINDS[0]) {
    "fresh" => listing(api, "articles", vec![("state", "fresh".to_owned())], page).await,
    _ => listing(api, "articles/latest", Vec::new(), page).await,
  }
}

/// An article's JSON, with its Markdown.
pub async fn json(api: &Api, article: &ArticleRef) -> Result<Value> {
  let path = match article {
    ArticleRef::Id(id) => format!("articles/{id}"),
    ArticleRef::Path { user, slug } => format!("articles/{user}/{slug}"),
  };
  api.get(&path, Vec::new()).await
}

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let v = json(api, &refs::article(arg)?).await?;
  Ok(parse::article(&v, &api.instance()))
}

/// A user's (or an organization's) published articles, newest first.
pub async fn by_user(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  let name = users::username(api, &refs::user(arg)?).await?;
  listing(api, "articles", vec![("username", name)], page).await
}

/// A tag's articles: the popular ones, or the most liked of a period.
pub async fn tagged(api: &Api, tag: &str, top: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  let mut query = vec![("tag", refs::tag(tag)?)];
  if let Some(days) = top.and_then(days) {
    query.push(("top", days.to_string()));
  }
  listing(api, "articles", query, page).await
}

/// Your unpublished articles.
pub async fn drafts(api: &Api, page: &PageReq) -> Result<Page<Post>> {
  api.require()?;
  listing(api, "articles/me/unpublished", Vec::new(), page).await
}

/// The community's tags, the most used first.
pub async fn tags(api: &Api, page: &PageReq) -> Result<Page<Collection>> {
  let (rows, next) = api.list("tags", Vec::new(), page).await?;
  let instance = api.instance();
  let tags = rows.iter().map(|row| parse::tag(row, &instance)).collect();
  Ok(Page::new(tags, next))
}
