//! SMZDM payloads → the shared model: the front page's JSON items
//! ([`item`]), the rows of HTML lists ([`rows`]), deal and article pages
//! ([`page`]) and their comments ([`comments`]). Times are Beijing's,
//! often written short (`16:41`, `10-18 22:41`, `3小时前`).

pub mod comments;
pub mod page;
pub mod rows;

use std::sync::LazyLock;

use atp_core::text::{date_only, html_to_text};
use atp_core::{Media, Metrics, Post, Price, User, Value, ValueExt, json};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use regex::Regex;

static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+(?:\.\d+)?").unwrap());
static AGO: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(\d+)\s*(秒|分钟|小时|天)前$").unwrap());
static MEMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/member/(\d+)").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
static NOISE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->|<script\b.*?</script>").unwrap());

/// The text of a short piece of HTML (a title, a price, a name): tags
/// dropped, entities read, spaces collapsed.
pub fn plain(html: &str) -> String {
  let text = html_to_text(&TAG.replace_all(html, " "));
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

static ANCHOR: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?i)</?a\b[^>]*>|<img\b[^>]*>").unwrap());

/// The text of a body or a comment, links left as their words (most are
/// the site's buying links) and pictures left out (they are listed as
/// media).
pub fn text(html: &str) -> String {
  html_to_text(&ANCHOR.replace_all(html, ""))
}

/// A page without its comments and scripts (templates of the page's own
/// lists among them).
pub fn visible(html: &str) -> std::borrow::Cow<'_, str> {
  NOISE.replace_all(html, "")
}

fn beijing() -> TimeZone {
  TimeZone::fixed(jiff::tz::offset(8))
}

/// A Beijing wall-clock time as a timestamp.
fn at(datetime: DateTime) -> Option<Timestamp> {
  datetime.to_zoned(beijing()).ok().map(|z| z.timestamp())
}

/// `HH:MM` on a day.
fn clock(date: Date, text: &str) -> Option<Timestamp> {
  let (hour, minute) = text.trim().split_once(':')?;
  let minute = minute.get(..2).unwrap_or(minute);
  at(date.at(hour.parse().ok()?, minute.parse().ok()?, 0, 0))
}

/// A time as the site writes it, in Beijing time: `刚刚`, `3分钟前`,
/// `2小时前`, `昨天 12:00`, `16:41` (today), `10-18 22:41` (this year),
/// `2025-10-18 22:41:22`, `2026-10-20T12:05:37`, or a day (`10-18`,
/// `2025-10-18`, `3天前`) without a time of day.
pub fn when(text: &str) -> Option<Timestamp> {
  let text = text.trim();
  // Ages (`3小时前`) are counted from the whole second.
  let now = Timestamp::now();
  let now = Timestamp::from_second(now.as_second()).unwrap_or(now);
  let today = now.to_zoned(beijing()).date();
  if text == "刚刚" {
    return Some(now);
  }
  if let Some(caps) = AGO.captures(text) {
    let n: i64 = caps[1].parse().ok()?;
    let secs = match &caps[2] {
      "秒" => n,
      "分钟" => n * 60,
      "小时" => n * 3600,
      _ => return date_only(today.checked_sub(jiff::Span::new().days(n)).ok()?),
    };
    return now.checked_sub(SignedDuration::from_secs(secs)).ok();
  }
  for (word, days) in [("昨天", 1), ("前天", 2)] {
    if let Some(rest) = text.strip_prefix(word) {
      return clock(today.checked_sub(jiff::Span::new().days(days)).ok()?, rest);
    }
  }
  if let Ok(ts) = text.parse::<Timestamp>() {
    return Some(ts);
  }
  full(text).or_else(|| short(text, today, now))
}

/// A time with its year: `2025-10-18 22:41:22`, `2026-10-20T12:05:37`, or
/// the day alone.
fn full(text: &str) -> Option<Timestamp> {
  let text = text.replace('T', " ");
  let datetime = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"]
    .iter()
    .find_map(|format| DateTime::strptime(format, &text).ok());
  match datetime {
    Some(datetime) => at(datetime),
    None => date_only(Date::strptime("%Y-%m-%d", &text).ok()?),
  }
}

/// A time without its year (`10-18 22:41`, `10-18`) or day (`16:41`),
/// read as the latest such time not ahead of now.
fn short(text: &str, today: Date, now: Timestamp) -> Option<Timestamp> {
  if !text.contains('-') {
    let ts = clock(today, text)?;
    return if ts > now {
      clock(today.yesterday().ok()?, text)
    } else {
      Some(ts)
    };
  }
  let (day, time) = text.split_once(' ').unwrap_or((text, ""));
  let (month, day) = day.split_once('-')?;
  let (month, day): (i8, i8) = (month.parse().ok()?, day.parse().ok()?);
  let on = |year: i16| {
    let date = Date::new(year, month, day).ok()?;
    if time.is_empty() {
      date_only(date)
    } else {
      clock(date, time)
    }
  };
  let ts = on(today.year())?;
  if ts > now + SignedDuration::from_hours(24) {
    on(today.year() - 1)
  } else {
    Some(ts)
  }
}

/// A price as the site writes it (`35.62元正文更低价34.7元（需用券）`,
/// `$29.99（约207元）`): its first figure, in the currency named with it;
/// the words as the label when there is more than the figure.
pub fn price(text: &str) -> Option<Price> {
  let text = text.trim();
  let found = NUMBER.find(text)?;
  let amount: f64 = found.as_str().parse().ok()?;
  let before = &text[..found.start()];
  let after = &text[found.end()..];
  let currency = if before.contains("HK$") || after.starts_with("港") {
    "HKD"
  } else if before.contains('$') || after.starts_with("美元") {
    "USD"
  } else if before.contains('€') || after.starts_with("欧元") {
    "EUR"
  } else if before.contains('£') || after.starts_with("英镑") {
    "GBP"
  } else if after.starts_with("日元") || before.contains('円') {
    "JPY"
  } else {
    "CNY"
  };
  let mut price = Price {
    amount,
    currency: currency.into(),
    ..Price::default()
  };
  let bare = [
    format!("{}元", found.as_str()),
    format!("¥{}", found.as_str()),
  ];
  if !bare.iter().any(|b| b == text) && text != found.as_str() {
    price.label = Some(text.to_owned());
  }
  Some(price)
}

/// What a link is: a deal (好价), an article (原创, 资讯) or a note (笔记).
pub fn kind(url: &str) -> &'static str {
  if url.contains("/talk/") {
    "note"
  } else if url.contains("www.smzdm.com/p/")
    || (url.contains("m.smzdm.com/p/") && !url.contains("post."))
  {
    "deal"
  } else {
    "article"
  }
}

/// A member (值友) by name and profile link.
pub fn member(name: &str, link: Option<&str>, avatar: Option<String>) -> User {
  let link = link.map(|l| l.trim_end_matches("pinglun").to_owned());
  User {
    id: link
      .as_deref()
      .and_then(|l| MEMBER.captures(l))
      .map(|c| c[1].to_owned())
      .unwrap_or_default(),
    name: name.trim().to_owned(),
    url: link,
    avatar,
    ..User::default()
  }
}

/// The link of a list item without the page's tracking fragment.
pub fn clean_link(url: &str) -> String {
  let url = url.split('#').next().unwrap_or(url).trim();
  match url.strip_prefix("//") {
    Some(rest) => format!("https://{rest}"),
    None => url.to_owned(),
  }
}

/// The id of a link (`184316466` of a deal, `ae6qdexk` of an article).
pub fn link_id(url: &str) -> String {
  url
    .trim_end_matches('/')
    .rsplit('/')
    .next()
    .unwrap_or_default()
    .to_owned()
}

/// An item of the front page's list (`/homepage/json_more`).
pub fn item(v: &Value) -> Post {
  let url = clean_link(&v.str("article_url").unwrap_or_default());
  let kind = kind(&url);
  let mut post = Post {
    id: v.str("article_id").unwrap_or_else(|| link_id(&url)),
    kind: kind.into(),
    title: v.str("article_title").map(|t| html_to_text(&t)),
    text: v
      .str("article_content")
      .map(|t| html_to_text(&t))
      .filter(|t| !t.is_empty()),
    author: v.str("article_referrals").map(|name| {
      member(
        &name,
        v.str("article_user_url").as_deref(),
        v.str("article_referrals_avatar"),
      )
    }),
    created_at: v
      .first_str(&["article_format_date", "article_date"])
      .as_deref()
      .and_then(when),
    metrics: Metrics {
      likes: v
        .u64("article_worthy")
        .or_else(|| v.u64("article_love_rating")),
      comments: v.u64("article_comment"),
      favorites: v.u64("article_collection"),
      ..Metrics::default()
    },
    price: v.str("article_price").as_deref().and_then(price),
    media: v.str("article_pic").map(Media::image).into_iter().collect(),
    tags: tags(v),
    raw: Some(v.clone()),
    url: Some(url),
    ..Post::default()
  };
  if let Some(n) = v.u64("article_unworthy") {
    post.metrics.other.insert("unworthy".into(), n);
  }
  item_extra(&mut post, v);
  dated_by_update(&mut post);
  post
}

/// Lists date a deal by its latest update (更新时间), an article by when it
/// was published.
pub fn dated_by_update(post: &mut Post) {
  if post.kind == "deal" {
    post.updated_at = post.created_at.take();
  }
}

fn tags(v: &Value) -> Vec<String> {
  let mut names: Vec<String> = Vec::new();
  for tag in v
    .list("article_tese_tags")
    .iter()
    .chain(v.list("article_show_tags"))
  {
    if let Some(name) = tag.first_str(&["name", "title"])
      && !names.contains(&name)
    {
      names.push(name);
    }
  }
  names
}

fn item_extra(post: &mut Post, v: &Value) {
  let extra = &mut post.extra;
  for (key, path) in [
    ("mall", "article_mall"),
    ("brand", "article_brand"),
    ("channel", "article_channel"),
  ] {
    if let Some(value) = v.str(path).filter(|s| !s.trim().is_empty()) {
      extra.insert(key.into(), json!(value));
    }
  }
  let categories: Vec<String> = v
    .list("category_names")
    .iter()
    .filter_map(|c| c.as_str().map(str::to_owned))
    .collect();
  if !categories.is_empty() {
    extra.insert("category".into(), json!(categories.join(" › ")));
  }
  for (key, path) in [
    ("sold_out", "article_is_sold_out"),
    ("expired", "article_is_timeout"),
  ] {
    if v.u64(path).is_some_and(|n| n > 0) {
      extra.insert(key.into(), json!(true));
    }
  }
}
