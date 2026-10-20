//! Article search through Sogou's WeChat search (搜狗微信,
//! weixin.sogou.com), the one public search of official-account articles.
//!
//! Its result pages are HTML: ten articles a page (`.news-list li`), ten
//! pages without a Sogou login. Each result carries its title, summary,
//! account name, the time as `timeConvert('<secs>')` and a Sogou link
//! (`/link?url=…`). A plain request to such a link is sent to Sogou's
//! anti-spider page (the results page's own script prepares the click),
//! so links come back as they are.
//!
//! Sogou answers clients it suspects with a redirect to `/antispider/`,
//! whose captcha sets the `SNUID` cookie. Requests go without cookies
//! (Sogou's must not mix with the mp.weixin.qq.com session), so a check
//! passed in a browser does not reach ATP: the error is final.

use std::sync::LazyLock;

use atp_core::text::{from_secs, html_to_text, one_line};
use atp_core::{Error, Page, PageReq, Post, Result, User, Value};
use regex::Regex;
use url::Url;

use crate::api::Api;
use crate::js::entities;

const SOGOU: &str = "https://weixin.sogou.com";
/// The last result page Sogou shows without a login.
const LAST_PAGE: u64 = 10;

static ITEM: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"(?s)<li id="sogou_vr_\d+_box_\d+"([^>]*)>(.*?)</li>"#).expect("valid")
});
static TITLE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"(?s)<h3>\s*<a\b[^>]*?\bhref="([^"]+)"[^>]*>(.*?)</a>"#).expect("valid")
});
static SUMMARY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"(?s)<p class="txt-info"[^>]*>(.*?)</p>"#).expect("valid"));
static ACCOUNT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"(?s)<span class="all-time-y2">(.*?)</span>"#).expect("valid"));
static TIME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"timeConvert\('(\d+)'\)").expect("valid"));
static COVER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"(?s)class="img-box".*?<img src="([^"]+)""#).expect("valid"));
static DOC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\bd="([^"]+)""#).expect("valid"));

/// One page of articles matching `query`; the cursor is the page number.
pub async fn articles(api: &Api, query: &str, req: &PageReq) -> Result<Page<Post>> {
  let query = query.trim();
  if query.is_empty() {
    return Err(Error::input("give words to search for"));
  }
  let page = req.number_or(1).max(1);
  let resp = api
    .ctx
    .http
    .get(format!("{SOGOU}/weixin"))
    .query("type", 2)
    .query("query", query)
    .query("ie", "utf8")
    .query("page", page)
    .header(
      "accept",
      "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    )
    .no_cookies()
    .no_redirects()
    .send()
    .await?;
  if resp.status.is_redirection() {
    return Err(redirected(resp.header("location").unwrap_or_default()));
  }
  let html = resp.check()?.text();
  if html.contains("/antispider/") && !html.contains("news-list") {
    return Err(antispider(&format!("{SOGOU}/antispider/")));
  }
  let posts: Vec<Post> = ITEM
    .captures_iter(&html)
    .filter_map(|m| item(&m[1], &m[2]))
    .collect();
  if posts.is_empty() && !html.contains("news-list") && !no_results(&html) {
    return Err(Error::upstream(
      "Sogou answered without a result list; its page may have changed",
    ));
  }
  let more = page < LAST_PAGE && html.contains("id=\"sogou_next\"");
  Ok(Page::new(posts, more.then(|| (page + 1).to_string())))
}

/// A redirect: the anti-spider page, or somewhere ATP does not follow.
fn redirected(location: &str) -> Error {
  let absolute = match Url::parse(SOGOU).and_then(|base| base.join(location)) {
    Ok(mut url) => {
      let _ = url.set_scheme("https");
      url.to_string()
    }
    Err(_) => location.to_owned(),
  };
  if location.contains("antispider") {
    return antispider(&absolute);
  }
  Error::upstream(format!("Sogou sent the search elsewhere: {absolute}"))
    .with_hint("Sogou shows ten result pages without a login; ask for an earlier page")
}

fn antispider(page: &str) -> Error {
  Error::verification("Sogou's WeChat search asks for a captcha (anti-spider check)")
    .with_url(page)
    .with_hint(
      "Sogou suspects automated searches from this network: wait a while (an hour or more) \
       before searching again, or search in a browser",
    )
    .unclearable()
}

/// Sogou's page for a search without results.
fn no_results(html: &str) -> bool {
  html.contains("noresult_part1_container") || html.contains("没有找到相关")
}

/// One result: `attrs` are the `<li>`'s attributes, `body` its content.
fn item(attrs: &str, body: &str) -> Option<Post> {
  let title = TITLE.captures(body)?;
  let link = format!("{SOGOU}{}", entities(&title[1]));
  let account = ACCOUNT
    .captures(body)
    .map(|m| text(&m[1]))
    .filter(|name| !name.is_empty());
  let mut post = Post {
    id: DOC
      .captures(attrs)
      .map(|m| m[1].to_owned())
      .unwrap_or_else(|| link.clone()),
    kind: "article".into(),
    title: Some(text(&title[2])).filter(|t| !t.is_empty()),
    text: SUMMARY
      .captures(body)
      .map(|m| text(&m[1]))
      .filter(|t| !t.is_empty()),
    url: Some(link),
    author: account.map(|name| User {
      name,
      ..User::default()
    }),
    created_at: TIME
      .captures(body)
      .and_then(|m| m[1].parse().ok())
      .and_then(from_secs),
    ..Post::default()
  };
  if let Some(cover) = COVER.captures(body).and_then(|m| cover(&m[1])) {
    post.extra.insert("cover".into(), Value::from(cover));
  }
  post.extra.insert("source".into(), Value::from("sogou"));
  Some(post)
}

/// Text of an HTML fragment on one line (Sogou marks the matched words).
fn text(html: &str) -> String {
  one_line(&html_to_text(html)).trim().to_owned()
}

/// The article's cover image behind Sogou's thumbnail proxy (its `url`).
fn cover(src: &str) -> Option<String> {
  let src = entities(src);
  let src = match src.strip_prefix("//") {
    Some(rest) => format!("https://{rest}"),
    None => src,
  };
  let url = Url::parse(&src)
    .ok()?
    .query_pairs()
    .find(|(k, _)| k == "url")
    .map(|(_, v)| v.into_owned())?;
  Some(url.replacen("http://", "https://", 1))
}
