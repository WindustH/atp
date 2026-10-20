//! The rows of the site's HTML lists (`li.feed-row-wide`, the search's
//! results among them): title and link, price, mall, time, author of an
//! article, 值 / 不值 votes or likes, saves and comments, picture,
//! summary and labels.

use std::sync::LazyLock;

use atp_core::text::parse_count;
use atp_core::{Media, Metrics, Post, json};
use regex::Regex;

use super::{clean_link, dated_by_update, kind, link_id, member, plain, price, visible, when};

const ROW: &str = "<li class=\"feed-row-wide";

fn re(pattern: &str) -> Regex {
  Regex::new(pattern).unwrap()
}

/// Event handlers, whose quoted scripts may hold `>`.
static HANDLERS: LazyLock<Regex> = LazyLock::new(|| re(r#"(?i)\son[a-z]+\s*=\s*"[^"]*""#));
static TITLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="feed-block-title[^"]*"[^>]*>\s*<a\b([^>]*)>(.*?)</a>"#));
static HREF: LazyLock<Regex> = LazyLock::new(|| re(r#"\bhref="([^"]+)""#));
static TITLE_ATTR: LazyLock<Regex> = LazyLock::new(|| re(r#"\btitle="([^"]+)""#));
static ARTICLE: LazyLock<Regex> = LazyLock::new(|| re(r#"articleid="\d+_([0-9A-Za-z]+)""#));
static PRICE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="z-highlight[^"]*"[^>]*>(.*?)</div>"#));
static UP: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)price-btn-up.*?unvoted-wrap.*?<span>\s*([^<]*?)\s*</span>"#));
static DOWN: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)price-btn-down.*?unvoted-wrap.*?<span>\s*([^<]*?)\s*</span>"#));
static LIKES: LazyLock<Regex> = LazyLock::new(|| re(r#"title="点赞\s*(\d+)""#));
static FAVORITES: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)title="收藏(?:数)?\s*(\d+)"|feed-btn-fav[^>]*>.*?<span>\s*([^<]*?)\s*</span>"#)
});
static COMMENTS: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)title="评论数\s*(\d+)"|feed-btn-comment[^>]*>(.*?)</a>"#));
static TIME: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="feed-block-extras"[^>]*>\s*([^<]*?)\s*<"#));
static MALL: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="(?:article-mall|feed-block-mall)"[^>]*>\s*([^<]*?)\s*<"#));
static AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)class="feed-little-img"[^>]*?href="([^"]*)"[^>]*>.*?<img[^>]*?\ssrc="([^"]+)".*?<span>([^<]+)</span>"#,
  )
});
static IMG: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="z-feed-img".*?<img\b[^>]*?\ssrc="([^"]+)""#));
static DESC: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="feed-block-descripe"[^>]*>(.*?)</div>"#));
static TAGS: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="feed-block-tags"[^>]*>(.*?)</span>"#));
static LABEL: LazyLock<Regex> = LazyLock::new(|| re(r#"<i[^>]*>([^<]+)</i>"#));

/// Every row of a list page.
pub fn rows(html: &str) -> Vec<Post> {
  visible(html).split(ROW).skip(1).filter_map(row).collect()
}

fn first<'a>(re: &Regex, text: &'a str) -> Option<&'a str> {
  re.captures(text)
    .and_then(|c| c.iter().skip(1).flatten().next())
    .map(|m| m.as_str())
}

fn count(re: &Regex, text: &str) -> Option<u64> {
  first(re, text).and_then(|t| parse_count(&plain(t)))
}

/// One row: `None` for rows without a title link (ads, banners).
fn row(chunk: &str) -> Option<Post> {
  let chunk = HANDLERS.replace_all(chunk, "");
  let caps = TITLE.captures(&chunk)?;
  let url = clean_link(first(&HREF, &caps[1])?);
  let title = first(&TITLE_ATTR, &caps[1])
    .map(plain)
    .unwrap_or_else(|| plain(&caps[2]));
  if title.is_empty() {
    return None;
  }
  let mut post = Post {
    id: first(&ARTICLE, &chunk)
      .map(str::to_owned)
      .unwrap_or_else(|| link_id(&url)),
    kind: kind(&url).into(),
    title: Some(title),
    text: first(&DESC, &chunk)
      .map(|d| plain(d).trim_end_matches("阅读全文").trim().to_owned())
      .filter(|d| !d.is_empty()),
    author: AUTHOR
      .captures(&chunk)
      .map(|c| member(&plain(&c[3]), Some(&c[1]), Some(c[2].to_owned()))),
    created_at: first(&TIME, &chunk).and_then(when),
    price: first(&PRICE, &chunk).and_then(|p| price(&plain(p))),
    metrics: Metrics {
      likes: count(&UP, &chunk).or_else(|| count(&LIKES, &chunk)),
      favorites: count(&FAVORITES, &chunk),
      comments: count(&COMMENTS, &chunk),
      ..Metrics::default()
    },
    media: first(&IMG, &chunk)
      .map(|src| Media::image(clean_link(src)))
      .into_iter()
      .collect(),
    url: Some(url),
    ..Post::default()
  };
  if let Some(n) = count(&DOWN, &chunk) {
    post.metrics.other.insert("unworthy".into(), n);
  }
  if let Some(mall) = first(&MALL, &chunk).map(plain).filter(|m| !m.is_empty()) {
    post.extra.insert("mall".into(), json!(mall));
  }
  if let Some(tags) = first(&TAGS, &chunk) {
    for label in LABEL.captures_iter(tags).map(|c| plain(&c[1])) {
      if !label.is_empty() && !post.tags.contains(&label) {
        post.tags.push(label);
      }
    }
  }
  dated_by_update(&mut post);
  Some(post)
}
