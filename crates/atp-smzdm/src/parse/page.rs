//! Deal (好价) and article pages: what they show at the top and in their
//! body, with the times and counts of their structured data (`ld+json`).

use std::sync::LazyLock;

use atp_core::text::parse_count;
use atp_core::{Media, Metrics, Post, Price, Value, ValueExt, json};
use regex::Regex;

use super::{kind, link_id, member, plain, price, text, when};

fn re(pattern: &str) -> Regex {
  Regex::new(pattern).unwrap()
}

static LD_JSON: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<script type="application/ld\+json">\s*(.*?)\s*</script>"#));
static DEAL_TITLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<h1 class="title J_title">(.*?)</h1>"#));
static PRICE_NUM: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="price-large">\s*<span class="num">([^<]*)</span>"#));
static PRICE_DESC: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="price-desc[^"]*">\s*([^<]*?)\s*<"#));
static PRICE_WAS: LazyLock<Regex> = LazyLock::new(|| re(r#"class="line-through">\s*([^<]*?)\s*<"#));
static MALL: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)class="canal">.*?class="item-subtitle">\s*<span>\s*([^<]*?)\s*</span>"#)
});
static AUTHOR: LazyLock<Regex> = LazyLock::new(|| {
  re(
    r#"(?s)J_author_info">\s*(?:<a href="([^"]+)".*?)?<img src="([^"]+)"[^>]*class="author-img".*?<span class="name">([^<]*)</span>"#,
  )
});
static BODY: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<article class="txt-detail">(.*?)</article>"#));
static WORTHY: LazyLock<Regex> = LazyLock::new(|| re(r#"id="rating_worthy_num">\s*(\d+)\s*<"#));
static UNWORTHY: LazyLock<Regex> = LazyLock::new(|| re(r#"id="rating_unworthy_num">\s*(\d+)\s*<"#));
static FAVORITES: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)class="fav J_zhi_like_fav"[^>]*>.*?<span>\s*(\d+)\s*</span>"#));
static COMMENTS: LazyLock<Regex> =
  LazyLock::new(|| re(r#"<em class="commentNum">\s*(\d+)\s*</em>"#));
static CRUMB: LazyLock<Regex> = LazyLock::new(|| re(r#"itemprop="title">([^<]+)<"#));
static LABEL: LazyLock<Regex> = LazyLock::new(|| re(r#"<span class="label ">([^<]+)</span>"#));
static ORDER_IMG: LazyLock<Regex> =
  LazyLock::new(|| re(r#"class="order-img[^"]*"((?s:.*?))</div>"#));
static REF: LazyLock<Regex> = LazyLock::new(|| re(r#"\bref="([^"]+)""#));
static ARTICLE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<article id="articleId"[^>]*>(.*?)</article>"#));
static ARTICLE_SKIP: LazyLock<Regex> = LazyLock::new(|| {
  re(r#"(?s)<h1\b.*?</h1>|<span class="xilie">.*?</span>\s*</span>|<input[^>]*>"#)
});
static XILIE: LazyLock<Regex> =
  LazyLock::new(|| re(r#"(?s)<span class="xilie">(.*?)</span>\s*</span>"#));
static IMG: LazyLock<Regex> = LazyLock::new(|| re(r#"<img\b[^>]*?\ssrc="(https?://[^"]+)""#));

fn first<'a>(re: &Regex, text: &'a str) -> Option<&'a str> {
  re.captures(text).and_then(|c| c.get(1)).map(|m| m.as_str())
}

fn number(re: &Regex, text: &str) -> Option<u64> {
  first(re, text).and_then(|n| n.trim().parse().ok())
}

/// The structured data of a page: every `ld+json` object.
fn ld_json(html: &str) -> Vec<Value> {
  LD_JSON
    .captures_iter(html)
    .filter_map(|c| serde_json::from_str(&c[1]).ok())
    .collect()
}

/// The pictures of a body, in order, once each.
fn images(html: &str) -> Vec<String> {
  let mut out: Vec<String> = Vec::new();
  for caps in IMG.captures_iter(html) {
    let src = caps[1].to_owned();
    // The site's own icons (labels, levels) are not the post's pictures.
    let icon = ["static.smzdm.com", "res.smzdm.com"]
      .iter()
      .any(|host| src.contains(host));
    if !icon && !out.contains(&src) {
      out.push(src);
    }
  }
  out
}

/// A page: a deal's or an article's, by its link.
pub fn page(html: &str, url: &str) -> Option<Post> {
  match kind(url) {
    "deal" => deal(html, url),
    _ => article(html, url),
  }
}

/// A deal (好价): title, price, mall, who found it, the text, the 值 / 不值
/// votes, saves and comments.
pub fn deal(html: &str, url: &str) -> Option<Post> {
  let title = first(&DEAL_TITLE, html).map(plain)?;
  let data = ld_json(html)
    .into_iter()
    .find(|v| !v.at("pubDate").is_null());
  let data = data.unwrap_or(Value::Null);
  let body = first(&BODY, html).unwrap_or_default();
  let mut media: Vec<Media> = data
    .list("images")
    .iter()
    .filter_map(Value::as_str)
    .filter(|s| !s.is_empty())
    .map(Media::image)
    .collect();
  media.extend(images(body).into_iter().map(Media::image));
  if let Some(block) = first(&ORDER_IMG, html) {
    media.extend(REF.captures_iter(block).map(|c| Media::image(&c[1])));
  }
  let mut post = Post {
    id: link_id(url),
    kind: "deal".into(),
    title: Some(title),
    text: Some(text(body)).filter(|t| !t.is_empty()),
    author: AUTHOR.captures(html).map(|c| {
      member(
        &plain(&c[3]),
        c.get(1).map(|m| m.as_str()),
        Some(c[2].to_owned()),
      )
    }),
    created_at: data.str("pubDate").as_deref().and_then(when),
    updated_at: data.str("upDate").as_deref().and_then(when),
    metrics: Metrics {
      likes: number(&WORTHY, html),
      comments: number(&COMMENTS, html),
      favorites: number(&FAVORITES, html),
      ..Metrics::default()
    },
    price: deal_price(html),
    media,
    tags: LABEL.captures_iter(html).map(|c| plain(&c[1])).collect(),
    url: Some(url.to_owned()),
    ..Post::default()
  };
  if let Some(n) = number(&UNWORTHY, html) {
    post.metrics.other.insert("unworthy".into(), n);
  }
  if let Some(mall) = first(&MALL, html).map(plain).filter(|m| !m.is_empty()) {
    post.extra.insert("mall".into(), json!(mall));
  }
  let crumbs: Vec<String> = CRUMB
    .captures_iter(html)
    .map(|c| plain(&c[1]))
    .filter(|c| c != "首页")
    .collect();
  if !crumbs.is_empty() {
    post
      .extra
      .insert("category".into(), json!(crumbs.join(" › ")));
  }
  Some(post)
}

/// The price at the top of a deal: the figure, the words beside it and
/// the crossed-out price.
fn deal_price(html: &str) -> Option<Price> {
  let figure = first(&PRICE_NUM, html)?.trim();
  let mut price = price(&format!("{figure}元"))?;
  price.label = first(&PRICE_DESC, html)
    .map(plain)
    .filter(|d| !d.is_empty());
  price.original = first(&PRICE_WAS, html).and_then(|was| super::price(was).map(|p| p.amount));
  Some(price)
}

/// An article (原创, 资讯, 笔记): title, author, the body as text with its
/// pictures, likes, saves and comments, its topics.
pub fn article(html: &str, url: &str) -> Option<Post> {
  let data = ld_json(html)
    .into_iter()
    .find(|v| v.str("@type").as_deref() == Some("Article"))?;
  let body = ARTICLE_SKIP.replace_all(first(&ARTICLE, html).unwrap_or_default(), "");
  let body_text = text(&body);
  let stat = |kind: &str| {
    data
      .list("interactionStatistic")
      .iter()
      .find(|s| s.str("interactionType").is_some_and(|t| t.ends_with(kind)))
      .and_then(|s| s.u64("userInteractionCount"))
  };
  let favorites = first(&XILIE, html).and_then(|x| {
    plain(x)
      .split_whitespace()
      .find_map(|part| part.strip_suffix("收藏").and_then(parse_count))
  });
  let mut media: Vec<Media> = images(&body).into_iter().map(Media::image).collect();
  if media.is_empty() {
    media.extend(
      data
        .list("image")
        .iter()
        .filter_map(|i| i.str("url"))
        .map(Media::image),
    );
  }
  Some(Post {
    id: link_id(url),
    kind: kind(url).into(),
    title: data.str("headline"),
    text: Some(body_text).filter(|t| !t.is_empty()),
    author: data
      .str("author.name")
      .map(|name| member(&name, data.str("author.url").as_deref(), None)),
    created_at: data.str("datePublished").as_deref().and_then(when),
    updated_at: data.str("dateModified").as_deref().and_then(when),
    metrics: Metrics {
      likes: stat("LikeAction"),
      comments: stat("CommentAction"),
      favorites,
      ..Metrics::default()
    },
    media,
    tags: data
      .list("about")
      .iter()
      .filter_map(|a| a.str("name"))
      .collect(),
    url: Some(url.to_owned()),
    raw: Some(data.clone()),
    ..Post::default()
  })
}
