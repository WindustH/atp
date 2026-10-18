//! Community posts: search (`/api/sparta/pc/search`), a post's page data
//! (`detail/moment-data/detail/<uuid>`, `detail/content-data/detail/<id>`),
//! the hot lists, the 面经 list (`job-experience/experience/job/list`), the
//! home page's 内推 tab, and liking (`/sns/like`, `/sns/del_like`).

use std::time::Duration;

use atp_core::{Action, Error, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::{Api, Body, Route};
use crate::filters::{self, Career};
use crate::parse::{self, CONTENT, MOMENT};
use crate::refs::{self, PostRef};

/// `search --sort` of posts: newest first, most popular; relevance by default.
pub const SORTS: &[&str] = &["new", "hot"];

/// `hot --category`: hot posts, hot searches, hot topics, the home feed.
pub const HOT_CATEGORIES: &[&str] = &["posts", "searches", "topics", "recommend"];

/// Posts per page of the home feed and the 内推 tab.
const PAGE: u64 = 20;
/// How long the number and author of a moment are kept: they never change.
const TARGET_TTL: Duration = Duration::from_secs(30 * 24 * 3600);

/// A parser of one entry of a hot list.
type Parser = fn(&Value) -> Option<Post>;

/// The posts of a `{current, totalPage, records}` answer.
fn feed_page(v: &Value, page: u64) -> Page<Post> {
  let items: Vec<Post> = v.list("records").iter().filter_map(parse::record).collect();
  let current = v.u64("current").unwrap_or(page);
  let more = v.u64("totalPage").is_some_and(|pages| current < pages);
  let cursor = (more && !items.is_empty()).then(|| (current + 1).to_string());
  Page::new(items, cursor)
}

pub async fn search(
  api: &Api,
  keyword: &str,
  sort: Option<&str>,
  req: &PageReq,
) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let order = match sort {
    Some("new") => "create",
    Some("hot") => "hot",
    _ => "",
  };
  let body = json!({"type": "post", "query": keyword, "page": page, "order": order});
  let v = api
    .json(Route::Gateway, "/api/sparta/pc/search", &body)
    .await?;
  Ok(feed_page(&v, page))
}

/// The page data of a moment or a long post.
async fn detail(api: &Api, post: &PostRef) -> Result<Value> {
  let path = match post {
    PostRef::Moment(uuid) => format!("/api/sparta/detail/moment-data/detail/{uuid}"),
    PostRef::Content(id) => format!("/api/sparta/detail/content-data/detail/{id}"),
    PostRef::Job(_) => return Err(Error::input("not a post")),
  };
  let v = api.get(Route::Gateway, &path, &[]).await?;
  if v.str("id").is_none() {
    return Err(Error::not_found(format!(
      "Nowcoder shows no post at {}",
      post.url()
    )));
  }
  Ok(v)
}

pub async fn read(api: &Api, post: &PostRef) -> Result<Post> {
  let kind = match post {
    PostRef::Moment(_) => MOMENT,
    _ => CONTENT,
  };
  let v = detail(api, post).await?;
  parse::detail(&v, kind)
    .ok_or_else(|| Error::upstream(format!("Nowcoder sent no post for {}", post.url())))
}

/// What the community's actions on a post take: its number, its content
/// type (74 moments, 250 long posts) and, when asked for, its author.
pub struct Target {
  pub id: String,
  pub kind: i64,
  pub author: Option<String>,
  pub url: String,
}

/// A moment's number and author, from the cache or its page data.
async fn moment(api: &Api, post: &PostRef) -> Result<(String, Option<String>)> {
  let key = format!(
    "nowcoder-moment-{}",
    post.url().rsplit('/').next().unwrap_or_default()
  );
  let store = &api.ctx.store;
  if let Some(v) = store.cache_get::<Value>(&key, TARGET_TTL)
    && let Some(id) = v.str("id")
  {
    return Ok((id, v.str("author")));
  }
  let v = detail(api, post).await?;
  let id = v.str("id").unwrap_or_default();
  let author = v.first_str(&["userId", "userBrief.userId"]);
  store.cache_put(&key, &json!({"id": id, "author": author}));
  Ok((id, author))
}

/// The post a comment, like … is for; `author` reads a long post's page
/// for its author (moments always tell it).
pub async fn target(api: &Api, arg: &str, author: bool) -> Result<Target> {
  let post = refs::post(arg)?;
  let url = post.url();
  match &post {
    PostRef::Job(_) => Err(Error::input(
      "a job has no comments or likes on Nowcoder; give a post (a /feed/main/detail/ or /discuss/ link)",
    )),
    PostRef::Moment(_) => {
      let (id, author) = moment(api, &post).await?;
      Ok(Target {
        id,
        kind: MOMENT,
        author,
        url,
      })
    }
    PostRef::Content(id) => {
      let author = match author {
        true => detail(api, &post)
          .await?
          .first_str(&["authorId", "userBrief.userId"]),
        false => None,
      };
      Ok(Target {
        id: id.clone(),
        kind: CONTENT,
        author,
        url,
      })
    }
  }
}

pub async fn hot(api: &Api, category: Option<&str>, req: &PageReq) -> Result<Page<Post>> {
  let (path, list, parse): (&str, &str, Parser) = match category.unwrap_or("posts") {
    "posts" => (
      "/api/sparta/hot-search/top-hot-pc",
      "result",
      parse::hot_post,
    ),
    "searches" => (
      "/api/sparta/hot-search/hot-content",
      "hotQuery",
      parse::trend,
    ),
    "topics" => ("/api/sparta/subject/hot-subject", "result", parse::topic),
    "recommend" => return recommended(api, req).await,
    other => {
      return Err(Error::input(format!(
        "unknown hot category: {other} (one of: {})",
        HOT_CATEGORIES.join(", ")
      )));
    }
  };
  let v = api.get(Route::Gateway, path, &[]).await?;
  Ok(Page::last(v.list(list).iter().filter_map(parse).collect()))
}

/// The home page's recommended feed (推荐).
async fn recommended(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let params = [("page", page.to_string()), ("size", PAGE.to_string())];
  let v = api
    .get(Route::Gateway, "/api/sparta/home/recommend", &params)
    .await?;
  Ok(feed_page(&v, page))
}

/// What the 面经 list is narrowed to.
pub struct Experience {
  /// Company ids.
  pub companies: Vec<String>,
  /// The career category; the first of the site's when none is given.
  pub career: Option<Career>,
  pub newest: bool,
}

/// A company's id for the 面经 list: its number, or its name looked up in
/// the list's company suggestions.
pub async fn experience_company(api: &Api, name: &str) -> Result<String> {
  let name = name.trim();
  if !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(name.to_owned());
  }
  let v = api
    .get(
      Route::Gateway,
      "/api/sparta/job-experience/company/name/suggest",
      &[("name", name.to_owned())],
    )
    .await?;
  let found = v.list("result");
  let pick = found
    .iter()
    .find(|c| c.str("companyName").as_deref() == Some(name))
    .or_else(|| found.first());
  pick
    .and_then(|c| c.first_str(&["companyId", "id"]))
    .ok_or_else(|| Error::input(format!("Nowcoder knows no company {name} in its 面经")))
}

/// 面经: interview write-ups by career and company, as the 面经 page lists
/// them (综合, or the newest first).
pub async fn experience(api: &Api, filter: &Experience, req: &PageReq) -> Result<Page<Post>> {
  let career = match &filter.career {
    Some(career) => career.clone(),
    None => filters::all_careers(api)
      .await?
      .into_iter()
      .next()
      .ok_or_else(|| Error::upstream("Nowcoder lists no career categories"))?,
  };
  let page = req.number_or(1);
  let companies: Vec<u64> = filter
    .companies
    .iter()
    .filter_map(|c| c.parse().ok())
    .collect();
  let body = json!({
    "companyList": companies,
    "jobId": career.id.parse::<u64>().unwrap_or_default(),
    "level": career.level,
    "order": if filter.newest { 3 } else { 1 },
    "page": page,
    "isNewJob": true,
  });
  let v = api
    .json(
      Route::Gateway,
      "/api/sparta/job-experience/experience/job/list",
      &body,
    )
    .await?;
  Ok(feed_page(&v, page))
}

/// The 内推 tab of the home page: referral posts (needs a login).
pub async fn referral(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let params = [
    ("tabId", "861".to_owned()),
    ("categoryType", "1".to_owned()),
    ("pageNo", page.to_string()),
    ("pageSize", PAGE.to_string()),
  ];
  let v = api
    .own(Route::Gateway, "/api/sparta/home/tab/content", &params)
    .await?;
  Ok(feed_page(&v, page))
}

/// Like (点赞) a post or take the like back.
pub async fn like(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let target = target(api, arg, false).await?;
  let path = if undo { "/sns/del_like" } else { "/sns/like" };
  let form = [("id", target.id.clone()), ("type", target.kind.to_string())];
  api.write(Route::Site, path, &[], Body::Form(&form)).await?;
  let action = if undo { "unlike" } else { "like" };
  let id = target.url.rsplit('/').next().unwrap_or_default().to_owned();
  Ok(Action::done(action, &id).with_url(target.url))
}
