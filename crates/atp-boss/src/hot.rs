//! Hot jobs: the 热招职位 box of the site's city pages
//! (`www.zhipin.com/<city>/`, and the home page for the city the site
//! places you in), which answer plain requests without the browser. The
//! box groups about a hundred jobs under nine kinds of work (销售,
//! 服务业, 技术 …). The cards name the job, its city, experience,
//! degree and keywords, and the company with its industry and funding
//! stage; their pay is left empty there.

use std::sync::LazyLock;

use atp_core::text::html_to_text;
use atp_core::{Error, Page, PageReq, Post, Result, User, json};
use regex::Regex;

use crate::api::{Api, WWW};
use crate::parse::{company_keys, salary};
use crate::{filters, refs};

/// The city pages the home page links: the city's code and the page's
/// name (苏州 is `suzhou`, not 宿州; 福州 is `fuzhou`, not 抚州).
const CITY_PAGES: &[(&str, &str)] = &[
  ("101010100", "beijing"),
  ("101020100", "shanghai"),
  ("101030100", "tianjin"),
  ("101110100", "xian"),
  ("101190400", "suzhou"),
  ("101200100", "wuhan"),
  ("101190100", "nanjing"),
  ("101180100", "zhengzhou"),
  ("101120200", "qingdao"),
  ("101210100", "hangzhou"),
  ("101230200", "xiamen"),
  ("101250100", "changsha"),
  ("101270100", "chengdu"),
  ("101280100", "guangzhou"),
  ("101280600", "shenzhen"),
  ("101220100", "hefei"),
  ("101040100", "chongqing"),
  ("101120100", "jinan"),
  ("101280800", "foshan"),
  ("101281600", "dongguan"),
  ("101290100", "kunming"),
  ("101240100", "nanchang"),
  ("101090100", "shijiazhuang"),
  ("101210400", "ningbo"),
  ("101230100", "fuzhou"),
];

/// What a card shows in place of the city for work done from home.
const REMOTE: &str = "在家办公";

/// The page of `--category`: a city by name, pinyin or code, or the home
/// page.
fn page_of(city: Option<&str>) -> Result<String> {
  let Some(city) = city.map(str::trim).filter(|c| !c.is_empty()) else {
    return Ok(format!("{WWW}/"));
  };
  let code = filters::city(city)?;
  let (_, page) = CITY_PAGES.iter().find(|(c, _)| *c == code).ok_or_else(|| {
    let pages: Vec<&str> = CITY_PAGES.iter().map(|(_, page)| *page).collect();
    Error::input(format!(
      "BOSS Zhipin has no city page of hot jobs for {city}; these cities have one: {}",
      pages.join(", ")
    ))
  })?;
  Ok(format!("{WWW}/{page}/"))
}

/// The first capture of `re` in `html`, as text.
fn one(re: &Regex, html: &str) -> Option<String> {
  let text = html_to_text(re.captures(html)?.get(1)?.as_str());
  Some(text).filter(|t| !t.is_empty())
}

/// The texts of the `<span>`s of `html`.
fn spans(html: &str) -> Vec<String> {
  static SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<span[^>]*>([^<]*)</span>").expect("valid pattern"));
  SPAN
    .captures_iter(html)
    .map(|c| html_to_text(&c[1]))
    .filter(|t| !t.is_empty())
    .collect()
}

/// The patterns a card is read with.
struct Card {
  job: Regex,
  security: Regex,
  name: Regex,
  salary: Regex,
  facts: Regex,
  company: Regex,
  company_name: Regex,
  logo: Regex,
  industry: Regex,
  stage: Regex,
}

impl Card {
  fn new() -> Self {
    let re = |pattern: &str| Regex::new(pattern).expect("valid pattern");
    Self {
      job: re(r#"href="/job_detail/([A-Za-z0-9_~-]+)\.html""#),
      security: re(r#"data-securityid="([^"]+)""#),
      name: re(r#"<p class="name">([^<]*)</p>"#),
      salary: re(r#"<p class="salary">([^<]*)</p>"#),
      facts: re(r#"(?s)<p class="job-text">(.*?)</p>"#),
      company: re(r#"href="/gongsi/([A-Za-z0-9_~-]+)\.html""#),
      company_name: re(r#"<span class="name">([^<]*)</span>"#),
      logo: re(r#"<img src="([^"]+)""#),
      industry: re(r#"<span class="type">([^<]*)</span>"#),
      stage: re(r#"<span class="level">([^<]*)</span>"#),
    }
  }

  fn company(&self, html: &str) -> Option<User> {
    let id = self.company.captures(html)?[1].to_owned();
    let mut user = User {
      url: Some(refs::company_url(&id)),
      name: one(&self.company_name, html).unwrap_or_else(|| id.clone()),
      avatar: self
        .logo
        .captures(html)
        .map(|c| c[1].to_owned())
        .filter(|l| l.starts_with("http")),
      id,
      ..User::default()
    };
    for (key, re) in [("industry", &self.industry), ("stage", &self.stage)] {
      if let Some(value) = one(re, html) {
        user.extra.insert(key.into(), json!(value));
      }
    }
    Some(user)
  }

  /// One `<li>` of the box, under the kind of work `category`.
  fn post(&self, html: &str, category: Option<&str>) -> Option<Post> {
    let id = self.job.captures(html)?[1].to_owned();
    let security = self.security.captures(html).map(|c| c[1].to_owned());
    let facts = self
      .facts
      .captures(html)
      .map(|c| spans(&c[1]))
      .unwrap_or_default();
    let mut post = Post {
      url: Some(refs::job_link(&id, security.as_deref(), None)),
      kind: "job".into(),
      title: one(&self.name, html),
      tags: facts.iter().skip(3).cloned().collect(),
      id,
      ..Post::default()
    };
    let author = self.company(html);
    let extra = &mut post.extra;
    if let Some(company) = &author {
      company_keys(extra, company);
    }
    if let Some(pay) = one(&self.salary, html) {
      salary(extra, &pay);
    }
    for (key, value) in ["city", "experience", "degree"].iter().zip(&facts) {
      if *key == "city" && value == REMOTE {
        extra.insert("remote".into(), json!(true));
      } else {
        extra.insert((*key).into(), json!(value));
      }
    }
    if let Some(category) = category {
      extra.insert("category".into(), json!(category));
    }
    post.author = author;
    Some(post)
  }
}

/// The cards of a page's 热招职位 box, each with its kind of work.
fn cards(html: &str) -> Vec<Post> {
  let Some(start) = html.find("hot-job-box") else {
    return Vec::new();
  };
  let html = &html[start..];
  let tabs = html
    .split_once("<h3>")
    .and_then(|(_, rest)| rest.split_once("</h3>"))
    .map(|(tabs, _)| spans(tabs))
    .unwrap_or_default();
  let card = Card::new();
  let mut posts = Vec::new();
  // Each kind of work is a `<ul>` of the box, in the order of the tabs.
  for (i, list) in html
    .split("<ul")
    .skip(1)
    .take(tabs.len().max(1))
    .enumerate()
  {
    let list = list.split("</ul>").next().unwrap_or_default();
    let category = tabs.get(i).map(String::as_str);
    posts.extend(
      list
        .split("<li")
        .skip(1)
        .filter_map(|li| card.post(li, category)),
    );
  }
  posts
}

/// `hot [--category CITY]`: the hot jobs of a city's page, or of the home
/// page (the city the site places you in by your address). The pages are
/// public and asked for without the session's cookies.
pub async fn hot(api: &Api, city: Option<&str>, req: &PageReq) -> Result<Page<Post>> {
  let url = page_of(city)?;
  let resp = api
    .ctx
    .http
    .get(&url)
    .header(
      "accept",
      "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    )
    .no_cookies()
    .no_redirects()
    .send()
    .await?;
  // Requests without cookies cannot carry a check passed in a browser.
  if resp.status.is_redirection() {
    return Err(
      Error::verification("BOSS Zhipin sent the page to its security check")
        .with_url(url)
        .with_hint("try again later")
        .unclearable(),
    );
  }
  let items = cards(&resp.check()?.text());
  if items.is_empty() {
    return Err(Error::upstream(format!(
      "BOSS Zhipin's page {url} came without its hot jobs (the page may have changed)"
    )));
  }
  Ok(atp_core::paging::slice(items, req))
}
