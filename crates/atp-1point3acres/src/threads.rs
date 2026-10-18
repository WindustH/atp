//! Threads: the hot list and the newest threads (`/api/threads?type=…`, as
//! the site's discover and home pages list them), a board's threads
//! (`/api/forums/<fid>/threads`), one thread read whole
//! (`/api/v3/home-threads/<tid>`), and with a login the search
//! (`/api/v2/search`) and a user's threads (`/api/users/<uid>/threads`).
//! Lists page by number (`pg`), twenty threads a page.

use atp_core::output::note;
use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// Threads a page, as the site's pages ask for them.
const PER_PAGE: usize = 20;
/// What lists of the discover and home pages carry with each thread.
const LIST_INCLUDES: &str = "images,forum_name,topic_tag,last_reply";

/// One page of threads under `threads`: the next page while pages come
/// full, or while `total` says more are left.
async fn page(
  api: &Api,
  path: &str,
  mut params: Vec<(&str, String)>,
  req: &PageReq,
) -> Result<Page<Post>> {
  let number = req.number_or(1);
  params.push(("ps", PER_PAGE.to_string()));
  params.push(("pg", number.to_string()));
  let v = api.get(path, &params).await?;
  let threads: Vec<Post> = v.list("threads").iter().map(parse::thread).collect();
  let more = match v.u64("total") {
    Some(total) => number * (PER_PAGE as u64) < total,
    None => threads.len() >= PER_PAGE,
  };
  let next = (more && !threads.is_empty()).then(|| (number + 1).to_string());
  Ok(Page::new(threads, next))
}

/// A list of the site's front pages: `hot` (热门) or `new` (最新主题).
async fn front(api: &Api, kind: &str, req: &PageReq) -> Result<Page<Post>> {
  let params = vec![
    ("with_post", "1".to_owned()),
    ("type", kind.to_owned()),
    ("includes", LIST_INCLUDES.to_owned()),
  ];
  page(api, "/api/threads", params, req).await
}

pub async fn hot(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  front(api, "hot", req).await
}

pub async fn newest(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  front(api, "new", req).await
}

/// How a board's threads are ordered, as the board pages offer it.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum Order {
  /// 最新发帖: newest threads first
  #[default]
  New,
  /// 最新回帖: latest replies first (the site's own default for most boards)
  Replied,
  /// 最热在前: hottest first
  Hot,
  /// 只看精华: only threads marked 精华
  Digest,
}

impl Order {
  fn param(self) -> &'static str {
    match self {
      Self::New => "time_desc",
      Self::Replied => "post_desc",
      Self::Hot => "hot_desc",
      Self::Digest => "digest",
    }
  }
}

/// The threads of a board, of one thread type (`typeid`) if asked.
pub async fn board(
  api: &Api,
  fid: u64,
  order: Order,
  kind: Option<u64>,
  req: &PageReq,
) -> Result<Page<Post>> {
  let mut params = vec![
    ("order", order.param().to_owned()),
    ("includes", "images,topic_tag,forum_name,summary".to_owned()),
    ("with_total", "1".to_owned()),
  ];
  if let Some(kind) = kind {
    params.push(("typeid", kind.to_string()));
  }
  page(api, &format!("/api/forums/{fid}/threads"), params, req).await
}

/// A thread with its first post as the text.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let tid = refs::thread(arg)?;
  let v = api.get(&format!("/api/v3/home-threads/{tid}"), &[]).await?;
  let thread = v.at("thread");
  if thread.is_null() {
    return Err(Error::not_found(format!(
      "1Point3Acres has no thread {tid}"
    )));
  }
  let post = parse::thread(thread);
  warn_hidden(&post);
  Ok(post)
}

/// Say on stderr when part of a thread is kept from this reader.
fn warn_hidden(post: &Post) {
  if post.extra.get("hidden").and_then(Value::as_bool) != Some(true) {
    return;
  }
  let credits = post
    .extra
    .get("hidden_credits")
    .and_then(Value::as_u64)
    .map(|n| format!(" (积分 above {n})"))
    .unwrap_or_default();
  note(&format!(
    "1Point3Acres keeps part of this thread from readers without enough points{credits}; \
     ATP shows what this session may see"
  ));
}

/// `search`: threads by words (needs a login).
pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<Post>> {
  let words = query.keyword.trim();
  if words.is_empty() {
    return Err(Error::input("search for what? give some words"));
  }
  let params = vec![
    ("content", words.to_owned()),
    ("forum_group", String::new()),
    ("order", String::new()),
    ("includes", String::new()),
  ];
  page(api, "/api/v2/search", params, req).await
}

/// `user-posts`: the threads a user started (needs a login).
pub async fn by_user(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let uid = refs::user(arg)?;
  let params = vec![("includes", "forum_name".to_owned())];
  page(api, &format!("/api/users/{uid}/threads"), params, req).await
}
