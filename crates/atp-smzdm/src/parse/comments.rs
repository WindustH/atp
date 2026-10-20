//! The comments of a deal's or an article's page: the 精彩 (hot) and 最新
//! (newest) blocks, each comment (`li.comment-main-list-item`) with the
//! replies shown under it (`.comment-main-list-item-child`).

use std::sync::LazyLock;

use atp_core::{Comment, json};
use regex::Regex;

use super::{member, plain, text, when};

fn re(pattern: &str) -> Regex {
  Regex::new(pattern).unwrap()
}

const ITEM: &str = "<li class=\"comment-main-list-item\"";
const CHILD: &str = "<div class=\"comment-main-list-item-child";
const REPLIES: &str = "class=\"comment-main-list-item-content-reply\"";

static ID: LazyLock<Regex> = LazyLock::new(|| re(r#"data-comment-id="(\d+)""#));
static NAME: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"class="comment-(?:child-)?user-name"[^>]*?href="([^"]*)"[^>]*>([^<]*)</a>"#)
});
static RECEIVER: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="comment-child-receive-name"[^>]*>([^<]*)</a>"#));
static AVATAR: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="comment-main-list-item-avatar-head" src="([^"]+)""#));
static TEXT: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="comment-main-list-item-content-comment[^"]*"\s*>(.*?)</div>"#));
static INFO: LazyLock<Regex> = LazyLock::new(|| re(r#"(?s)class="content-info"\s*>(.*?)</div>"#));
static SPAN: LazyLock<Regex> = LazyLock::new(|| re(r#"<span[^>]*>([^<]*)</span>"#));
static LIKES: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)J_zan comment-main-list-item-content-info-zan"[^>]*>\s*<span>\s*(\d*)\s*</span>"#)
});
static PICTURE: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)comment-main-list-item-content-img.*?<img[^>]*?\s(?:data-src|src)="(https?://[^"]+)""#)
});

/// The block of comments a page shows for an order: `commentTabBlockHot`
/// (精彩) or `commentTabBlockNew` (最新).
pub fn block<'a>(html: &'a str, id: &str) -> &'a str {
  let marker = format!("id=\"{id}\"");
  let Some(start) = html.find(&marker) else {
    return "";
  };
  let rest = &html[start..];
  let end = rest[marker.len()..]
    .find("class=\"tab_info")
    .map(|i| i + marker.len())
    .unwrap_or(rest.len());
  &rest[..end]
}

/// The comments of a block, each with the replies shown under it.
pub fn list(block: &str) -> Vec<Comment> {
  block.split(ITEM).skip(1).filter_map(item).collect()
}

fn item(chunk: &str) -> Option<Comment> {
  let (own, replies) = chunk.split_once(REPLIES).unwrap_or((chunk, ""));
  let mut comment = one(own)?;
  comment.replies = replies.split(CHILD).skip(1).filter_map(one).collect();
  if !comment.replies.is_empty() {
    comment.reply_count = Some(comment.replies.len() as u64);
  }
  Some(comment)
}

/// One comment or reply: author, text (pictures as `[image] …` lines),
/// time, place and likes.
fn one(chunk: &str) -> Option<Comment> {
  let id = ID.captures(chunk)?[1].to_owned();
  let author = NAME.captures(chunk).map(|c| {
    member(
      &plain(&c[2]),
      Some(&c[1]),
      AVATAR.captures(chunk).map(|a| a[1].to_owned()),
    )
  });
  let mut body = TEXT
    .captures(chunk)
    .map(|c| text(&c[1]))
    .unwrap_or_default();
  for caps in PICTURE.captures_iter(chunk) {
    if !body.is_empty() {
      body.push('\n');
    }
    body.push_str(&format!("[image] {}", &caps[1]));
  }
  let info: Vec<String> = INFO
    .captures(chunk)
    .map(|c| {
      SPAN
        .captures_iter(&c[1])
        .map(|s| plain(&s[1]))
        .filter(|s| !s.is_empty() && s != "·")
        .collect()
    })
    .unwrap_or_default();
  let mut comment = Comment {
    id,
    author,
    text: body,
    created_at: info.first().and_then(|t| when(t)),
    likes: LIKES.captures(chunk).map(|c| c[1].parse().unwrap_or(0)),
    reply_to: RECEIVER.captures(chunk).map(|c| plain(&c[1])),
    location: info.get(1).cloned(),
    ..Comment::default()
  };
  if chunk.contains("class=\"text-scale\">作者<") {
    comment.extra.insert("author_of_post".into(), json!(true));
  }
  Some(comment)
}
