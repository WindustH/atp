//! Posts: the listings of the site (the jobs channel `/jobs_channel/posts`,
//! the home page and categories `/posts`, the categories you follow),
//! search (`/posts/search`), a user's posts, reading one (`/posts/<id>`),
//! and the writes on posts: upvote (`like`) and save (`favorite`, 收藏).

use atp_core::http::Method;
use atp_core::{Action, Error, Page, PageReq, Post, Query, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// `hot --category`: the jobs channel (all job posts, or one way of
/// hiring), every post (the home page), or one discussion category.
pub const HOT_CATEGORIES: &[&str] = &[
  "jobs",
  "community",
  "matched",
  "featured",
  "all",
  "talk",
  "ai",
  "abroad",
  "stories",
  "freelance",
  "feedback",
  "official",
];

/// `search --sort`: Eleduck's relevance (the default), newest first, hot.
pub const SEARCH_SORTS: &[&str] = &["relevance", "new", "hot"];
/// `search --filter jobs`: job posts only (只看招聘).
pub const SEARCH_FILTERS: &[&str] = &["jobs"];

/// Where a `hot` listing comes from.
enum Listing {
  /// The jobs channel, all of it or one `recruitment_type`.
  Jobs(Option<&'static str>),
  /// The home page: every category.
  All,
  /// One category by its number.
  Category(u64),
}

fn listing(category: &str) -> Listing {
  match category {
    "community" => Listing::Jobs(Some("jd")),
    "matched" => Listing::Jobs(Some("jobs_channel")),
    "featured" => Listing::Jobs(Some("talent")),
    "all" => Listing::All,
    "talk" => Listing::Category(1),
    "ai" => Listing::Category(29),
    "abroad" => Listing::Category(10),
    "stories" => Listing::Category(15),
    "freelance" => Listing::Category(23),
    "feedback" => Listing::Category(28),
    "official" => Listing::Category(14),
    _ => Listing::Jobs(None),
  }
}

/// What the jobs channel is narrowed to (`jobs`).
#[derive(Debug, Default)]
pub struct JobFilter {
  /// `recruitment_type`: `jd`, `jobs_channel` or `talent`.
  pub kind: Option<&'static str>,
  /// Tag ids, one per tag group at most.
  pub tags: Vec<String>,
  /// `-published_at` for the newest first; the site's order otherwise.
  pub sort: Option<&'static str>,
}

/// The posts of a `{posts, pager}` answer.
fn posts_page(v: &Value) -> Page<Post> {
  let items = v.list("posts").iter().map(parse::post).collect();
  Page::new(items, parse::next_page(v))
}

pub async fn hot(api: &Api, category: Option<&str>, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1).to_string();
  match listing(category.unwrap_or("jobs")) {
    Listing::Jobs(kind) => {
      let filter = JobFilter {
        kind,
        ..JobFilter::default()
      };
      jobs(api, &filter, req).await
    }
    Listing::All => Ok(posts_page(&api.get("/posts", &[("page", page)]).await?)),
    Listing::Category(id) => {
      let params = [("page", page), ("category", id.to_string())];
      Ok(posts_page(&api.get("/posts", &params).await?))
    }
  }
}

/// The jobs channel (机会), as `/jobs-channel` lists it.
pub async fn jobs(api: &Api, filter: &JobFilter, req: &PageReq) -> Result<Page<Post>> {
  let mut params = vec![
    ("page", req.number_or(1).to_string()),
    ("talent_included", "true".to_owned()),
  ];
  if let Some(kind) = filter.kind {
    params.push(("recruitment_type", kind.to_owned()));
  }
  if let Some(sort) = filter.sort {
    params.push(("sort", sort.to_owned()));
  }
  for tag in &filter.tags {
    params.push(("tags[]", tag.clone()));
  }
  Ok(posts_page(&api.get("/jobs_channel/posts", &params).await?))
}

/// Posts of the categories you follow.
pub async fn followed(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let params = [("page", req.number_or(1).to_string())];
  let v = api
    .get_own("/profile/follows/category_posts", &params)
    .await?;
  Ok(posts_page(&v))
}

pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<Post>> {
  let mut params = vec![
    ("q", query.keyword.clone()),
    ("page", req.number_or(1).to_string()),
  ];
  let sort = query.sort.as_deref();
  let jobs_only = query.filter.as_deref() == Some("jobs");
  if jobs_only && sort == Some("hot") {
    return Err(Error::input(
      "Eleduck cannot sort a jobs-only search by `hot`; use `--sort new` or the default order",
    ));
  }
  match sort {
    Some("new") => params.push(("sort", "-published_at".to_owned())),
    Some("hot") => params.push(("sort", "hot".to_owned())),
    _ => {}
  }
  if jobs_only {
    params.push(("job", "true".to_owned()));
  }
  Ok(posts_page(&api.get("/posts/search", &params).await?))
}

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::post(arg)?;
  let v = api.get(&format!("/posts/{id}"), &[]).await?;
  let post = v.at("post");
  if post.str("id").is_none() {
    return Err(Error::not_found(format!("Eleduck has no post {id}")));
  }
  Ok(parse::post(post))
}

pub async fn user_posts(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let id = refs::user(arg)?;
  let params = [("page", req.number_or(1).to_string())];
  Ok(posts_page(
    &api.get(&format!("/users/{id}/posts"), &params).await?,
  ))
}

/// Upvote (点赞) a post, or take the upvote back.
pub async fn like(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::post(arg)?;
  let method = if undo { Method::DELETE } else { Method::POST };
  api
    .write(method, &format!("/posts/{id}/upvote"), None)
    .await?;
  let action = if undo { "unlike" } else { "like" };
  Ok(Action::done(action, &id).with_url(parse::post_url(&id)))
}

/// Save a post to your 收藏 (one list, no folders), or take it out.
pub async fn favorite(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::post(arg)?;
  let method = if undo { Method::DELETE } else { Method::POST };
  api
    .write(method, &format!("/posts/{id}/mark"), None)
    .await?;
  let action = if undo { "unfavorite" } else { "favorite" };
  Ok(Action::done(action, &id).with_url(parse::post_url(&id)))
}
