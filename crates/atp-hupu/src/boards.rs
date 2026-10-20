//! Boards (版块) and zones (分区): the boards the front page lists under
//! each zone (`window.$$data.pageData.categories`, kept a day), boards
//! found by their place on the site or their name, and a board's threads
//! (`/<board>`, `/<board>-postdate`, `/<board>-hot`, page N at `-N`).

use std::time::Duration;

use atp_core::{Collection, Error, Page, PageReq, Post, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

const CACHE_KEY: &str = "hupu-zones";
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);

/// The zones of the front page, each with its boards, from the cache or
/// the page.
async fn zones(api: &Api) -> Result<Vec<Value>> {
  let store = &api.ctx.store;
  if let Some(Value::Array(list)) = store.cache_get::<Value>(CACHE_KEY, CACHE_TTL) {
    return Ok(list);
  }
  let v = api.list_data("/").await?;
  let list = v.list("pageData.categories").to_vec();
  if !list.is_empty() {
    store.cache_put(CACHE_KEY, &Value::Array(list.clone()));
  }
  Ok(list)
}

/// Every board of every zone, in the site's order, each naming its zone.
pub async fn list(api: &Api) -> Result<Vec<Collection>> {
  let mut out: Vec<Collection> = Vec::new();
  for zone in zones(api).await? {
    let name = zone.str("name");
    for topic in zone.list("topics") {
      let board = parse::board(topic, name.as_deref());
      if !board.id.is_empty() && !out.iter().any(|b| b.id == board.id) {
        out.push(board);
      }
    }
  }
  Ok(out)
}

/// The place on the site (`history`) of the board an argument names: its
/// link or place, else its name among the boards.
pub async fn find(api: &Api, arg: &str) -> Result<String> {
  if let Some(place) = refs::board(arg) {
    return Ok(place);
  }
  let wanted = arg.trim();
  let boards = list(api).await?;
  let found = boards
    .iter()
    .find(|b| b.name == wanted)
    .or_else(|| boards.iter().find(|b| b.name.contains(wanted)));
  found.map(|b| b.id.clone()).ok_or_else(|| {
    Error::not_found(format!(
      "Hupu has no board {wanted} among those the front page lists (`boards`); give its link"
    ))
  })
}

/// How a board's threads are ordered, as the board's tabs offer it.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum Order {
  /// 最新回复: latest replies first (the board's own default)
  #[default]
  Replied,
  /// 最新发布: newest threads first
  New,
  /// 24小时榜: the board's hottest threads of the last day
  Hot,
}

impl Order {
  fn suffix(self) -> &'static str {
    match self {
      Self::Replied => "",
      Self::New => "-postdate",
      Self::Hot => "-hot",
    }
  }
}

/// One page of a board's threads (fifty a page).
pub async fn threads(api: &Api, place: &str, order: Order, req: &PageReq) -> Result<Page<Post>> {
  let number = req.number_or(1);
  let mut path = format!("/{place}{}", order.suffix());
  if number > 1 {
    path.push_str(&format!("-{number}"));
  }
  let v = api.list_data(&path).await?;
  let list = v.at("topic.threads");
  if list.is_null() {
    return Err(Error::not_found(format!(
      "{} is not a board of Hupu",
      parse::site_url(place)
    )));
  }
  let board = v.at("topic.topic");
  let posts: Vec<Post> = list
    .list("list")
    .iter()
    .map(|item| with_board(parse::thread(item), board))
    .collect();
  let total = list.u64("total").unwrap_or(number);
  let next = (number < total && !posts.is_empty()).then(|| (number + 1).to_string());
  Ok(Page::new(posts, next))
}

/// A thread of a board's list, with what the board page tells of the board.
fn with_board(mut post: Post, board: &Value) -> Post {
  let Some(entry) = post.extra.get_mut("board").and_then(Value::as_object_mut) else {
    return post;
  };
  for (key, path) in [
    ("zone", "cateName"),
    ("fid", "fid"),
    ("topic_id", "topicId"),
  ] {
    if let Some(value) = board.str(path) {
      entry.entry(key).or_insert(Value::String(value));
    }
  }
  post
}
