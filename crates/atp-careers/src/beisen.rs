//! Beisen (北森) career sites at `<tenant>.zhiye.com`, and companies'
//! own domains running them, through the requests their pages make:
//!
//! - the page carries the site (`var BSGlobal`: its portal id and the
//!   company), kept a day;
//! - `POST /api/Jobad/GetJobAdPageList`: the search (words, the kind of
//!   hiring as `Category` 1 社招, 2 校招, 3 实习, pages from 0);
//! - `GET /api/JobAd/GetJobAdInfo`: a job, by its id (a GUID) and kind,
//!   with the fields to show.
//!
//! Answers are `{Code: 200, Message, Data, Count}`.

use std::time::Duration;

use atp_core::http::Req;
use atp_core::{Error, Page, PageReq, Post, Result, Value, ValueExt, json};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::api::{self, Api};
use crate::jobs::{self, Filter, Kind};
use crate::parse::beisen as parse;
use crate::sites::Site;

/// Jobs per page.
const PAGE: u64 = 20;
/// How long a site's portal is kept.
const DAY: Duration = Duration::from_secs(24 * 3600);
/// The fields asked for, beside the title and text.
const FIELDS: &[&str] = &[
  "Category",
  "Kind",
  "LocId",
  "PostDate",
  "EndTime",
  "Salary",
  "HeadCount",
  "Org",
  "Degree",
  "YearsOfWorking",
];

/// What a site's page says about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portal {
  pub id: String,
  pub company: String,
}

async fn portal(api: &Api, site: &Site) -> Result<Portal> {
  let key = api::cache_key("beisen", &site.origin);
  if let Some(portal) = api.ctx.store.cache_get::<Portal>(&key, DAY) {
    return Ok(portal);
  }
  let page = api.page(&site.url()).await?;
  let portal = parse::portal(&page.text)
    .ok_or_else(|| Error::upstream(format!("{} is not a Beisen site ATP can read", site.url())))?;
  api.ctx.store.cache_put(&key, &portal);
  Ok(portal)
}

/// `Category` of a kind.
fn category(kind: Kind) -> u8 {
  match kind {
    Kind::Social => 1,
    Kind::Campus => 2,
    Kind::Intern => 3,
  }
}

fn headers<'a>(req: Req<'a>, site: &Site, referer: &str) -> Req<'a> {
  req
    .header("accept", "application/json")
    .header("x-requested-with", "xmlhttprequest")
    .header("langtype", "zh_CN")
    .header("origin", &site.origin)
    .header("referer", referer)
}

fn answer(v: Value, site: &Site) -> Result<Value> {
  match v.i64("Code") {
    Some(200) => Ok(v),
    code => Err(api::upstream(
      site.host(),
      v.str("Message"),
      code.unwrap_or_default(),
    )),
  }
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  for (set, what) in [
    (&filter.city, "city"),
    (&filter.batch, "batch"),
    (&filter.category, "category"),
  ] {
    if set.is_some() {
      return Err(jobs::unfiltered(site, what));
    }
  }
  let portal = portal(api, site).await?;
  let page = req.number_or(0);
  let mut body = json!({
    "PageIndex": page,
    "PageSize": PAGE,
    "KeyWords": filter.words,
    "SpecialType": 0,
    "PortalId": portal.id,
    "DisplayFields": FIELDS,
  });
  if let Some(kind) = filter.kind(site) {
    body["Category"] = json!([category(kind)]);
  }
  let referer = format!("{}/jobs", site.origin);
  let req = api
    .ctx
    .http
    .post(format!("{}/api/Jobad/GetJobAdPageList", site.origin))
    .json(&body);
  let v = answer(
    api.json(headers(req, site, &referer), &referer).await?,
    site,
  )?;
  let rows = v.list("Data");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(|row| parse::job(row, site, &portal))
    .map(|job| job.post(false))
    .collect();
  let more = !rows.is_empty() && (page + 1) * PAGE < v.u64("Count").unwrap_or(0);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

pub async fn read(api: &Api, site: &Site, id: &str) -> Result<Post> {
  let portal = portal(api, site).await?;
  let kind = site.kind.unwrap_or(Kind::Campus);
  let mut fields = vec!["jobAdName", "Duty", "Require"];
  fields.extend(FIELDS);
  let page = parse::job_url(site, id, Some(kind));
  let req = api
    .ctx
    .http
    .get(format!("{}/api/JobAd/GetJobAdInfo", site.origin))
    .query("_timestamp", Timestamp::now().as_millisecond())
    .query("jobAdId", id)
    .query("category", category(kind))
    .query("displayFields", json!(fields).to_string())
    .query("portalId", &portal.id);
  let v = api.json(headers(req, site, &page), &page).await?;
  let gone = || Error::not_found(format!("{page} shows no job (it may have been taken down)"));
  if v.i64("Code") == Some(500) {
    return Err(gone());
  }
  let v = answer(v, site)?;
  parse::job(v.at("Data"), site, &portal)
    .map(|job| job.post(true))
    .ok_or_else(gone)
}
