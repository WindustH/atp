//! Alibaba's career sites, one portal on two hosts, through the requests
//! its pages make: campus-talent.alibaba.com (campus jobs and
//! internships, by batch) and talent.alibaba.com (experienced hires).
//!
//! The page hands out the visit's token (`window.__sysconfig.__token__`)
//! with its cookies; every call carries it as `?_csrf=`:
//! `POST /searchCondition/listBatch` (batches: 2027届应届生, 日常实习生 …),
//! `/searchCondition/list` (a batch's cities and categories),
//! `/position/search` (words, a batch, cities, categories, page; campus
//! searches need a batch) and `/position/detail` (a job). Answers are
//! `{success, errorMsg, content}`.
//!
//! talent.alibaba.com's pages load Alibaba's baxia bot check; an answer
//! that is its check instead of data is reported, never passed.

use std::time::Duration;

use atp_core::{Collection, Error, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::{self, Api};
use crate::jobs::{self, Codes, Filter, Kind};
use crate::parse::alibaba as parse;
use crate::sites::Site;

pub const CAMPUS: &str = "https://campus-talent.alibaba.com";
pub const TALENT: &str = "https://talent.alibaba.com";
/// Jobs per page.
const PAGE: u64 = 20;
/// How long batches and a batch's filters are kept.
const DAY: Duration = Duration::from_secs(24 * 3600);

/// One of the two hosts.
struct Portal {
  origin: &'static str,
  /// The page handing out the token, the referer of every call.
  page: String,
  channel: &'static str,
  social: bool,
}

fn portal(site: &Site, kind: Option<Kind>) -> Portal {
  if kind.or(site.kind) == Some(Kind::Social) {
    Portal {
      origin: TALENT,
      page: format!("{TALENT}/off-campus/position-list?lang=zh"),
      channel: "group_official_site",
      social: true,
    }
  } else {
    Portal {
      origin: CAMPUS,
      page: format!("{CAMPUS}/campus/position"),
      channel: "new_campus_group_official_site",
      social: false,
    }
  }
}

/// The visit's token, from the page.
async fn token(api: &Api, portal: &Portal) -> Result<String> {
  if let Some(token) = api.token(portal.origin) {
    return Ok(token);
  }
  let page = api.page(&portal.page).await?;
  let token = parse::token(&page.text).ok_or_else(|| {
    Error::upstream(format!(
      "{} handed out no token on {} (the page may have changed, or a bot check answered)",
      api::host(portal.origin),
      portal.page
    ))
  })?;
  api.set_token(portal.origin, &token);
  Ok(token)
}

/// A call of the portal, its `content`.
async fn call(api: &Api, portal: &Portal, path: &str, body: Value) -> Result<Value> {
  let token = token(api, portal).await?;
  let req = api
    .ctx
    .http
    .post(format!("{}{path}", portal.origin))
    .query("_csrf", &token)
    .header("origin", portal.origin)
    .header("referer", &portal.page)
    .json(&body);
  let v = api.json(req, &portal.page).await?;
  if v.bool("success") == Some(true) {
    return Ok(v.at("content").clone());
  }
  Err(api::upstream(
    &api::host(portal.origin),
    v.str("errorMsg"),
    v.str("errorCode").unwrap_or_default(),
  ))
}

/// The campus site's batches, kept a day.
async fn batches(api: &Api, portal: &Portal) -> Result<Vec<Collection>> {
  let key = "alibaba-batches";
  if let Some(v) = api.ctx.store.cache_get::<Value>(key, DAY) {
    return Ok(parse::batches(&v));
  }
  let body = json!({"channel": portal.channel, "language": "zh"});
  let v = call(api, portal, "/searchCondition/listBatch", body).await?;
  api.ctx.store.cache_put(key, &v);
  Ok(parse::batches(&v))
}

/// The batch a campus search covers: the one asked for, else the first
/// of the kind (graduates by default).
async fn batch(api: &Api, portal: &Portal, filter: &Filter, kind: Option<Kind>) -> Result<String> {
  if let Some(batch) = &filter.batch {
    return Ok(batch.trim().to_owned());
  }
  let list = batches(api, portal).await?;
  let group = match kind {
    Some(Kind::Intern) => "internship",
    _ => "graduate",
  };
  list
    .iter()
    .find(|b| b.extra.get("group").and_then(Value::as_str) == Some(group))
    .or(list.first())
    .map(|b| b.id.clone())
    .ok_or_else(|| Error::not_found("campus-talent.alibaba.com lists no batch open now"))
}

/// A campus batch's cities and categories, kept a day.
async fn conditions(api: &Api, portal: &Portal, batch: &str) -> Result<Value> {
  let key = api::cache_key("alibaba-conditions", batch);
  if let Some(v) = api.ctx.store.cache_get::<Value>(&key, DAY) {
    return Ok(v);
  }
  let body = json!({"batchId": number(batch)?, "channel": portal.channel, "language": "zh"});
  let v = call(api, portal, "/searchCondition/list", body).await?;
  api.ctx.store.cache_put(&key, &v);
  Ok(v)
}

fn number(id: &str) -> Result<u64> {
  id.trim()
    .parse()
    .map_err(|_| Error::input(format!("not an Alibaba id: {id}")))
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let kind = filter.kind(site);
  let portal = portal(site, kind);
  let page = req.number_or(1);
  let mut body = json!({
    "channel": portal.channel,
    "language": "zh",
    "key": filter.words,
    "pageIndex": page,
    "pageSize": PAGE,
  });
  if portal.social {
    if filter.batch.is_some() || filter.category.is_some() || filter.city.is_some() {
      return Err(jobs::unfiltered(
        site,
        "batch, city or category for experienced hires",
      ));
    }
  } else {
    let batch = batch(api, &portal, filter, kind).await?;
    body["batchId"] = json!(number(&batch)?);
    if filter.city.is_some() || filter.category.is_some() {
      let v = conditions(api, &portal, &batch).await?;
      if let Some(city) = &filter.city {
        body["regions"] = json!(jobs::choose(
          &parse::conditions(&v, "workCity", "city"),
          city,
          site,
          "city"
        )?);
      }
      if let Some(category) = &filter.category {
        let list = parse::conditions(&v, "category", "category");
        body["subCategories"] = json!(jobs::choose(&list, category, site, "category")?);
      }
    }
  }
  let v = call(api, &portal, "/position/search", body).await?;
  let rows = v.list("datas");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(|row| parse::job(row, portal.social))
    .map(|job| job.post(false))
    .collect();
  let more = !rows.is_empty() && page * PAGE < v.u64("totalCount").unwrap_or(0);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

pub async fn read(api: &Api, site: &Site, id: &str) -> Result<Post> {
  let portal = portal(site, None);
  let body = json!({"id": number(id)?, "channel": portal.channel, "language": "zh"});
  let v = call(api, &portal, "/position/detail", body).await?;
  parse::job(&v, portal.social)
    .map(|job| job.post(true))
    .ok_or_else(|| {
      Error::not_found(format!(
        "{} shows no job {id} (it may have been taken down)",
        api::host(portal.origin)
      ))
    })
}

/// The campus site's batches, and a batch's cities and categories.
pub async fn codes(
  api: &Api,
  site: &Site,
  codes: Codes,
  kind: Option<Kind>,
) -> Result<Vec<Collection>> {
  let portal = portal(site, kind);
  if portal.social {
    return Err(jobs::no_codes(
      &Site {
        kind: Some(Kind::Social),
        ..site.clone()
      },
      codes,
    ));
  }
  if codes == Codes::Batches {
    return batches(api, &portal).await;
  }
  let batch = batch(api, &portal, &Filter::default(), kind).await?;
  let v = conditions(api, &portal, &batch).await?;
  Ok(match codes {
    Codes::Cities => parse::conditions(&v, "workCity", "city"),
    _ => parse::conditions(&v, "category", "category"),
  })
}
