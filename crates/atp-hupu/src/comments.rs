//! Replies (回帖): a thread's replies page by page as its pages show them
//! (`/<tid>-<n>.html`, twenty a page) or its 亮了 highlights (the replies
//! most lit, on its first page); with a login replying to a thread
//! (`createReply`) and lighting a reply (`reply/light`, taken back with
//! `reply/cancelLight`), as the thread pages send them.

use atp_core::{Action, Comment, Error, Page, PageReq, Reply, Result, Value, ValueExt, json};

use crate::api::{self, Api};
use crate::{parse, refs, threads};

/// `comments --sort`: the thread's order (oldest first), or its 亮了
/// highlights.
pub const SORTS: &[&str] = &["oldest", "lights"];

pub async fn list(
  api: &Api,
  arg: &str,
  sort: Option<&str>,
  req: &PageReq,
) -> Result<Page<Comment>> {
  let tid = refs::thread(arg)?;
  if sort == Some("lights") {
    let detail = threads::detail(api, &tid, 1).await?;
    let lights = detail.list("lights").iter().map(parse::reply).collect();
    return Ok(Page::last(lights));
  }
  let number = req.number_or(1);
  let detail = threads::detail(api, &tid, number).await?;
  let replies = detail.at("replies");
  let comments: Vec<Comment> = replies.list("list").iter().map(parse::reply).collect();
  let total = replies.u64("total").unwrap_or(number);
  let next = (number < total && !comments.is_empty()).then(|| (number + 1).to_string());
  Ok(Page::new(comments, next))
}

/// What the actions need of a thread: its board's numbers.
struct Board {
  topic_id: String,
  fid: String,
}

async fn board(api: &Api, tid: &str) -> Result<Board> {
  let detail = threads::detail(api, tid, 1).await?;
  let thread = detail.at("thread");
  let field = |paths: &[&str]| thread.first_str(paths).unwrap_or_default();
  Ok(Board {
    topic_id: field(&["topicId", "topic.topicId"]),
    fid: field(&["fid", "topic.fid"]),
  })
}

/// The text as the site's editor sends it: paragraphs of escaped text.
fn paragraphs(text: &str) -> String {
  let escape = |line: &str| {
    line
      .replace('&', "&amp;")
      .replace('<', "&lt;")
      .replace('>', "&gt;")
  };
  text
    .lines()
    .map(|line| format!("<p>{}</p>", escape(line)))
    .collect()
}

/// Reply to a thread, or with `--reply-to` quote one of its replies.
pub async fn add(api: &Api, arg: &str, reply: &Reply) -> Result<Action> {
  let tid = refs::thread(arg)?;
  let text = reply.text.trim();
  if text.is_empty() {
    return Err(Error::input("the reply is empty"));
  }
  let quote = reply.reply_to.as_deref().map(refs::reply).transpose()?;
  let board = board(api, &tid).await?;
  let mut body = json!({
    "topicId": board.topic_id,
    "content": paragraphs(text),
    "shumeiId": "",
    "deviceid": "",
    "tid": tid,
  });
  if let Some(quote) = &quote {
    body["quoteId"] = json!(quote);
  }
  let url = parse::thread_url(&tid);
  let v = api
    .post("/pcmapi/pc/bbs/v1/createReply", &body, &url)
    .await?;
  let action = Action::done("comment", &tid).with_url(url);
  Ok(match v.str("data.pid") {
    Some(pid) => action.with_id(pid),
    None => action,
  })
}

/// Light (亮) a reply of a thread, or take the light back.
pub async fn light(api: &Api, thread: &str, reply: &str, undo: bool) -> Result<Action> {
  let tid = refs::thread(thread)?;
  let pid = refs::reply(reply)?;
  let board = board(api, &tid).await?;
  let body = json!({
    "tid": tid,
    "pid": pid,
    "puid": api::cookie_or_empty(api, "ua"),
    "fid": board.fid,
    "shumei_id": api::cookie_or_empty(api, "smidV2"),
    "deviceid": "",
  });
  let (path, action) = if undo {
    ("/pcmapi/pc/bbs/v1/reply/cancelLight", "unlike_comment")
  } else {
    ("/pcmapi/pc/bbs/v1/reply/light", "like_comment")
  };
  let url = parse::thread_url(&tid);
  let done = Action::done(action, &pid).with_url(format!("{url}?pid={pid}"));
  match api.post(path, &body, &url).await {
    Ok(v) => Ok(match message(&v) {
      Some(message) => done.with_message(message),
      None => done,
    }),
    // 你已经点亮过这个回帖了: the reply was lit before.
    Err(e) if e.message.contains("已经") => Ok(done.with_message(e.message)),
    Err(e) => Err(e),
  }
}

fn message(v: &Value) -> Option<String> {
  v.first_str(&["msg", "message"])
    .filter(|m| !m.is_empty() && m != "success")
}
