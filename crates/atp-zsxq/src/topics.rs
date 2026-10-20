//! Topics (主题): one topic (`/v2/topics/<tid>`), a group's topics by the
//! app's tabs (`/v2/groups/<gid>/topics?scope=…`), the digests of your
//! groups (`hot`) and the feed of all of them (`/v2/dynamics`). Listings
//! go newest first and page by `end_time`, a millisecond before the last
//! topic.

use atp_core::output::note;
use atp_core::{Error, ErrorCode, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::{groups, parse, refs};

/// The tabs of a group, as `topics --scope` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum Scope {
  /// Every topic (全部)
  #[default]
  All,
  /// The digests (精华)
  Digests,
  /// The owner's topics (只看星主)
  Owner,
  /// Questions and answers (问答)
  Questions,
}

impl Scope {
  fn api(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::Digests => "digests",
      Self::Owner => "by_owner",
      Self::Questions => "questions",
    }
  }
}

/// The largest page the app asks for.
const PAGE: usize = 20;

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let tid = refs::topic(arg)?;
  api.need_login()?;
  let v = api.get(&format!("/v2/topics/{tid}"), &[]).await?;
  parse::topic(v.at("topic"))
    .ok_or_else(|| Error::not_found(format!("知识星球 has no topic {tid}")))
}

/// A page of a group's topics, newest first.
pub async fn list(api: &Api, gid: &str, scope: Scope, req: &PageReq) -> Result<Page<Post>> {
  api.need_login()?;
  let count = req.size_within(PAGE);
  let mut params = vec![
    ("scope", scope.api().to_owned()),
    ("count", count.to_string()),
  ];
  if let Some(end) = &req.cursor {
    params.push(("end_time", end.clone()));
  }
  let v = api
    .get(&format!("/v2/groups/{gid}/topics"), &params)
    .await?;
  let topics = v.list("topics");
  let posts = topics.iter().filter_map(parse::topic).collect();
  Ok(Page::new(posts, before(topics.last(), topics.len(), count)))
}

/// The cursor of a newest-first page: a millisecond before its last
/// item, when the page was full.
fn before(last: Option<&Value>, len: usize, count: usize) -> Option<String> {
  if len < count {
    return None;
  }
  parse::shifted(&last?.str("create_time")?, -1)
}

/// The digests (精华) of your groups, one group after another. The cursor
/// is the group's place in `groups` and, inside it, the `end_time`
/// (`2~2026-10-20T12:00:00.000+0800`). A group that refuses (an expired
/// membership) is skipped with a note.
pub async fn hot(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let ids = groups::ids(api).await?;
  let (index, end) = match req.cursor.as_deref() {
    None => (0, None),
    Some(cursor) => {
      let (index, end) = cursor.split_once('~').unwrap_or((cursor, ""));
      let index = index
        .parse::<usize>()
        .map_err(|_| Error::input(format!("not a cursor of `hot`: {cursor}")))?;
      (index, (!end.is_empty()).then(|| end.to_owned()))
    }
  };
  let Some(gid) = ids.get(index) else {
    return Ok(Page::last(Vec::new()));
  };
  let at = PageReq {
    cursor: end,
    size: req.size,
  };
  let page = match list(api, gid, Scope::Digests, &at).await {
    Ok(page) => page,
    Err(e) if e.code == ErrorCode::PermissionDenied => {
      note(&format!("skipped group {gid}: {}", e.message));
      Page::last(Vec::new())
    }
    Err(e) => return Err(e),
  };
  let next = match page.next_cursor {
    Some(end) => Some(format!("{index}~{end}")),
    None => (index + 1 < ids.len()).then(|| (index + 1).to_string()),
  };
  Ok(Page::new(page.items, next))
}

/// The feed of all your groups (动态, `scope=general`): new topics as
/// they come, newest first; each post's `extra.action` is the dynamic's.
pub async fn feed(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  api.need_login()?;
  let count = req.size_within(30);
  let mut params = vec![
    ("scope", "general".to_owned()),
    ("count", count.to_string()),
  ];
  if let Some(end) = &req.cursor {
    params.push(("end_time", end.clone()));
  }
  let v = api.get("/v2/dynamics", &params).await?;
  let dynamics = v.list("dynamics");
  let mut posts = Vec::new();
  let mut last = None;
  for dynamic in dynamics {
    let Some(topic) = dynamic_topic(dynamic) else {
      continue;
    };
    if let Some(mut post) = parse::topic(&topic) {
      if let Some(action) = dynamic.str("action") {
        post.extra.insert("action".into(), json!(action));
      }
      posts.push(post);
    }
    last = Some(topic);
  }
  let last = last.as_ref().or(dynamics.last());
  Ok(Page::new(posts, before(last, dynamics.len(), count)))
}

/// The topic of a dynamic, with the dynamic's time and group where the
/// topic lacks them, as the app reads it.
fn dynamic_topic(dynamic: &Value) -> Option<Value> {
  let mut topic = dynamic.get("topic").filter(|t| t.is_object())?.clone();
  if topic.str("create_time").is_none()
    && let Some(time) = dynamic.str("create_time")
  {
    topic["create_time"] = json!(time);
  }
  if let Some(group) = dynamic.get("group").filter(|g| g.is_object()) {
    topic["group"] = group.clone();
  }
  Some(topic)
}

/// A group's topics for someone without a login: what its public page
/// shows.
pub async fn preview(api: &Api, gid: &str) -> Result<Page<Post>> {
  let (_, topics) = groups::preview(api, gid).await?;
  note("not logged in: these are the topics the group's public page shows");
  Ok(Page::last(topics))
}
