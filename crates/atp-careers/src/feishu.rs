//! Feishu Hire (飞书招聘) career sites: ByteDance's own
//! (jobs.bytedance.com, `/campus` and `/experienced`) and any company's
//! website at `<tenant>.jobs.feishu.cn/<website>`, through the requests
//! their pages make:
//!
//! - the site's page carries its website (`js-websiteInfo`: the path the
//!   API takes as `website-path`, and the company), kept a day;
//! - `POST /api/v1/csrf/token` hands out the visit's CSRF token, sent
//!   back as `x-csrf-token`;
//! - `POST /api/v1/search/job/posts`: the job search (words, offset and
//!   limit, cities, categories, subjects, recruitment types);
//! - `GET /api/v1/job/posts/<id>`: a job;
//! - `GET /api/v1/config/job/filters/<portal type>`: the cities,
//!   categories and subjects (batches) of the search's filters.
//!
//! The portal type is the kind of website: 3 ByteDance's campus site, 2
//! its experienced-hire site, 6 a company's site. The pages also sign the
//! search and job requests (ByteDance's acrawler `_signature`); the sites
//! answer without it. Should one refuse an unsigned request, ATP says so
//! and stops: it does not sign.

use std::time::Duration;

use atp_core::http::{Method, Req};
use atp_core::{Collection, Error, Page, PageReq, Post, Result, Value, ValueExt, json};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::api::{self, Api};
use crate::jobs::{self, Codes, Filter, Kind};
use crate::parse::feishu as parse;
use crate::sites::{self, Site};

/// ByteDance's own site.
const BYTEDANCE: &str = "https://jobs.bytedance.com";
/// How long a website's description and filters are kept.
const DAY: Duration = Duration::from_secs(24 * 3600);
/// Jobs per page at most.
const PAGE_MAX: usize = 50;

/// What a website's page says about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Website {
  /// `website-path` (`campus`, `society`, `graduate`).
  path: String,
  company: String,
}

/// One website, ready for requests.
pub struct Portal {
  pub origin: String,
  /// The path of its links (`campus`, `experienced`, `graduate`).
  pub route: String,
  /// The path the API takes (`campus`, `society`, `graduate`).
  path: String,
  pub company: String,
  /// `portal_type`.
  kind: u8,
  /// `Portal-Channel`.
  channel: &'static str,
}

impl Portal {
  pub fn site_url(&self) -> String {
    format!("{}/{}", self.origin, self.route)
  }

  pub fn job_url(&self, id: &str) -> String {
    format!("{}/{}/position/{id}/detail", self.origin, self.route)
  }

  fn experienced(&self) -> bool {
    self.kind == 2
  }
}

/// ByteDance's site for the kind asked: experienced hires on
/// `/experienced`, campus jobs and internships on `/campus`.
fn by_kind(site: &Site, kind: Option<Kind>) -> Site {
  let mut site = site.clone();
  if site.origin == BYTEDANCE {
    match kind {
      Some(Kind::Social) => site.path = "experienced".into(),
      Some(_) => site.path = "campus".into(),
      None => {}
    }
  }
  site
}

async fn portal(api: &Api, site: &Site) -> Result<Portal> {
  let key = api::cache_key("feishu", &site.url());
  let website = match api.ctx.store.cache_get::<Website>(&key, DAY) {
    Some(website) => website,
    None => {
      let page = api.page(&site.url()).await?;
      let (path, company) = parse::website(&page.text).ok_or_else(|| {
        Error::upstream(format!(
          "{} is not a Feishu Hire website ATP can read",
          site.url()
        ))
      })?;
      let company = sites::company(site)
        .map(str::to_owned)
        .or(company)
        .unwrap_or_else(|| site.host().to_owned());
      let website = Website { path, company };
      api.ctx.store.cache_put(&key, &website);
      website
    }
  };
  let (kind, channel) = match (site.origin == BYTEDANCE, site.path.as_str()) {
    (true, "experienced") => (2, "office"),
    (true, _) => (3, "campus"),
    _ => (6, "saas-career"),
  };
  let route = if site.path.is_empty() {
    website.path.clone()
  } else {
    site.path.clone()
  };
  Ok(Portal {
    origin: site.origin.clone(),
    route,
    path: website.path,
    company: website.company,
    kind,
    channel,
  })
}

fn request<'a>(api: &'a Api, portal: &Portal, method: Method, path: &str) -> Req<'a> {
  api
    .ctx
    .http
    .request(method, format!("{}/api/v1{path}", portal.origin))
    .header("website-path", &portal.path)
    .header("portal-channel", portal.channel)
    .header("portal-platform", "pc")
    .header("origin", &portal.origin)
    .header("referer", format!("{}/position", portal.site_url()))
}

/// The visit's CSRF token.
async fn csrf(api: &Api, portal: &Portal) -> Result<String> {
  if let Some(token) = api.token(&portal.origin) {
    return Ok(token);
  }
  let req = request(api, portal, Method::POST, "/csrf/token").json(&json!({"portal_entrance": 1}));
  let v = answer(api.json(req, &portal.site_url()).await?, portal)?;
  let token = v
    .str("token")
    .ok_or_else(|| Error::upstream(format!("{} handed out no CSRF token", portal.origin)))?;
  api.set_token(&portal.origin, &token);
  Ok(token)
}

/// A call with the visit's token; a refused token is renewed once, as the
/// pages do.
async fn call(
  api: &Api,
  portal: &Portal,
  method: Method,
  path: &str,
  body: Option<&Value>,
) -> Result<Value> {
  for attempt in 0..2 {
    let token = csrf(api, portal).await?;
    let mut req = request(api, portal, method.clone(), path).header("x-csrf-token", &token);
    if let Some(body) = body {
      req = req.json(body);
    }
    let resp = req.send().await?;
    if matches!(resp.status.as_u16(), 403 | 405) {
      if resp.header("x-risk-tag").is_some() || attempt == 1 {
        return Err(refused(portal));
      }
      api.forget_token(&portal.origin);
      continue;
    }
    return answer(api::parse(&resp, &portal.site_url())?, portal);
  }
  Err(refused(portal))
}

fn refused(portal: &Portal) -> Error {
  Error::upstream(format!(
    "{} refused the request: its pages sign their requests (ByteDance's acrawler `_signature`), which ATP does not do",
    api::host(&portal.origin)
  ))
}

/// The `data` of an answer (`{code, message, data}`), or its error.
fn answer(v: Value, portal: &Portal) -> Result<Value> {
  match v.i64("code") {
    Some(0) => Ok(v.at("data").clone()),
    Some(-9000003) => Err(Error::not_found(format!(
      "{} has no website {}",
      api::host(&portal.origin),
      portal.route
    ))),
    code => Err(api::upstream(
      &api::host(&portal.origin),
      v.first_str(&["message", "error.message"]),
      code.unwrap_or_default(),
    )),
  }
}

/// `_random`, which the pages add to every `GET`.
fn random() -> String {
  (100_000 + Timestamp::now().as_millisecond().rem_euclid(900_000)).to_string()
}

/// What the search's filters offer, kept a day.
async fn filters(api: &Api, portal: &Portal) -> Result<Value> {
  let key = api::cache_key("feishu-filters", &portal.site_url());
  if let Some(v) = api.ctx.store.cache_get::<Value>(&key, DAY) {
    return Ok(v);
  }
  let path = format!("/config/job/filters/{}?_random={}", portal.kind, random());
  let v = call(api, portal, Method::GET, &path, None).await?;
  api.ctx.store.cache_put(&key, &v);
  Ok(v)
}

/// `recruitment_id_list` of a kind: Feishu Hire's own ids, 1 社招, 201
/// 校招 正式, 202 校招 实习. ByteDance's experienced site has social jobs only.
fn recruitment(portal: &Portal, kind: Option<Kind>) -> Vec<&'static str> {
  match kind {
    _ if portal.experienced() => vec![],
    Some(Kind::Campus) => vec!["201"],
    Some(Kind::Intern) => vec!["202"],
    Some(Kind::Social) => vec!["1"],
    None => vec![],
  }
}

fn search_body(portal: &Portal, filter: &Filter, offset: u64, limit: usize) -> Value {
  json!({
    "keyword": filter.words,
    "limit": limit,
    "offset": offset,
    "job_category_id_list": filter.category.iter().collect::<Vec<_>>(),
    "tag_id_list": [],
    "location_code_list": filter.city.iter().collect::<Vec<_>>(),
    "subject_id_list": filter.batch.iter().collect::<Vec<_>>(),
    "recruitment_id_list": recruitment(portal, filter.kind),
    "portal_type": portal.kind,
    "job_function_id_list": [],
    "storefront_id_list": [],
    "portal_entrance": 1,
  })
}

/// The filter with its city and category as the site's codes.
async fn coded(api: &Api, portal: &Portal, site: &Site, filter: &Filter) -> Result<Filter> {
  let mut filter = filter.clone();
  if filter.city.is_none() && filter.category.is_none() {
    return Ok(filter);
  }
  let v = filters(api, portal).await?;
  if let Some(city) = &filter.city {
    filter.city = Some(jobs::choose(&parse::cities(&v), city, site, "city")?);
  }
  if let Some(category) = &filter.category {
    let list = parse::categories(&v);
    filter.category = Some(jobs::choose(&list, category, site, "category")?);
  }
  Ok(filter)
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let site = by_kind(site, filter.kind);
  let portal = portal(api, &site).await?;
  let filter = Filter {
    kind: filter.kind(&site),
    ..coded(api, &portal, &site, filter).await?
  };
  let offset = req.number_or(0);
  let body = search_body(&portal, &filter, offset, req.size_within(PAGE_MAX));
  let v = call(api, &portal, Method::POST, "/search/job/posts", Some(&body)).await?;
  let rows = v.list("job_post_list");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(|row| parse::job(row, &portal))
    .map(|job| job.post(false))
    .collect();
  let seen = offset + rows.len() as u64;
  let more = !rows.is_empty() && seen < v.u64("count").unwrap_or(0);
  Ok(Page::new(items, more.then(|| seen.to_string())))
}

pub async fn read(api: &Api, site: &Site, id: &str) -> Result<Post> {
  let portal = portal(api, site).await?;
  let path = format!(
    "/job/posts/{id}?portal_type={}&with_recommend=false&_random={}",
    portal.kind,
    random()
  );
  let v = call(api, &portal, Method::GET, &path, None).await?;
  let detail = v.at("job_post_detail");
  parse::job(detail, &portal)
    .map(|job| job.post(true))
    .ok_or_else(|| {
      Error::not_found(format!(
        "{} shows no job {id} (it may have been taken down)",
        portal.site_url()
      ))
    })
}

/// `batches` (subjects), `cities` and `categories` of a website.
pub async fn codes(
  api: &Api,
  site: &Site,
  codes: Codes,
  kind: Option<Kind>,
) -> Result<Vec<Collection>> {
  let site = by_kind(site, kind);
  let portal = portal(api, &site).await?;
  let v = filters(api, &portal).await?;
  Ok(match codes {
    Codes::Batches => parse::subjects(&v),
    Codes::Cities => parse::cities(&v),
    Codes::Categories => parse::categories(&v),
  })
}
