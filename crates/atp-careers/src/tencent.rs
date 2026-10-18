//! Tencent's career sites, through the requests their pages make:
//!
//! - join.qq.com, campus jobs and internships (腾讯校招), every request
//!   with the `timestamp` its pages add: the projects
//!   (`/api/v1/position/getProjectMapping`: 2027校园招聘, 应届实习, 日常实习,
//!   青云计划 …), the search (`POST /api/v1/position/searchPosition`:
//!   words, projects by their mapping ids, cities, categories, page), a
//!   job (`/api/v1/jobDetails/getJobDetailsByPostId`), the cities and
//!   categories of the filters (`getPositionWorkCities`,
//!   `getPositionFamily`); answers are `{status: 0, data}`;
//! - careers.tencent.com, experienced hires (腾讯社招): the search
//!   (`/tencentcareer/api/post/Query`) and a job (`/tencentcareer/api/post/ByPostId`);
//!   answers are `{Code: 200, Data}`.

use std::time::Duration;

use atp_core::http::Req;
use atp_core::{Collection, Error, Page, PageReq, Post, Result, Value, ValueExt, json};
use jiff::Timestamp;

use crate::api::{self, Api};
use crate::jobs::{self, Codes, Filter, Kind};
use crate::parse::tencent as parse;
use crate::sites::Site;

pub const JOIN: &str = "https://join.qq.com";
pub const CAREERS: &str = "https://careers.tencent.com";
/// Jobs per page.
const PAGE: u64 = 20;
/// How long the projects, cities and categories are kept.
const DAY: Duration = Duration::from_secs(24 * 3600);

fn social(site: &Site, kind: Option<Kind>) -> bool {
  kind.or(site.kind) == Some(Kind::Social)
}

fn stamp(req: Req<'_>) -> Req<'_> {
  req.query("timestamp", Timestamp::now().as_millisecond())
}

/// A `GET` of join.qq.com, its `data`.
async fn join_get(api: &Api, path: &str, params: &[(&str, String)]) -> Result<Value> {
  let req = api
    .ctx
    .http
    .get(format!("{JOIN}{path}"))
    .queries(params.iter().map(|(k, v)| (*k, v)))
    .header("referer", format!("{JOIN}/post.html"));
  join_answer(api.json(stamp(req), JOIN).await?)
}

fn join_answer(v: Value) -> Result<Value> {
  match v.i64("status") {
    Some(0) => Ok(v.at("data").clone()),
    code => Err(api::upstream(
      "join.qq.com",
      v.str("message"),
      code.unwrap_or_default(),
    )),
  }
}

/// What join.qq.com keeps a day: the projects, cities and categories.
async fn kept(api: &Api, path: &str) -> Result<Value> {
  let key = api::cache_key("tencent", path);
  if let Some(v) = api.ctx.store.cache_get::<Value>(&key, DAY) {
    return Ok(v);
  }
  let v = join_get(api, path, &[("lang", "zh-cn".into())]).await?;
  api.ctx.store.cache_put(&key, &v);
  Ok(v)
}

/// The mapping ids of the projects a search covers.
async fn projects(api: &Api, filter: &Filter, kind: Option<Kind>) -> Result<Vec<u64>> {
  if let Some(batch) = &filter.batch {
    let id = batch.trim().parse().map_err(|_| {
      Error::input(format!(
        "not a project of join.qq.com: {batch} (see `atp careers batches --site tencent`)"
      ))
    })?;
    return Ok(vec![id]);
  }
  let v = kept(api, "/api/v1/position/getProjectMapping").await?;
  let wanted = match kind {
    Some(Kind::Campus) => Some(1),
    Some(Kind::Intern) => Some(2),
    _ => None,
  };
  Ok(
    parse::projects(&v)
      .into_iter()
      .filter(|p| wanted.is_none_or(|w| p.recruit == w))
      .map(|p| p.id)
      .collect(),
  )
}

async fn campus(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let mut body = json!({
    "projectIdList": [],
    "projectMappingIdList": projects(api, filter, filter.kind(site)).await?,
    "keyword": filter.words,
    "bgList": [],
    "workCountryType": 0,
    "workCityList": [],
    "recruitCityList": [],
    "positionFidList": [],
    "pageIndex": page,
    "pageSize": PAGE,
  });
  if let Some(city) = &filter.city {
    let cities = parse::cities(&kept(api, "/api/v1/position/getPositionWorkCities").await?);
    let code = jobs::choose(&cities, city, site, "city")?;
    let abroad = cities
      .iter()
      .any(|c| c.id == code && c.description.is_some());
    body["workCountryType"] = json!(if abroad { 2 } else { 1 });
    body["workCityList"] = json!([code]);
  }
  if let Some(category) = &filter.category {
    let list = parse::categories(&kept(api, "/api/v1/position/getPositionFamily").await?);
    body["positionFidList"] = json!([jobs::choose(&list, category, site, "category")?]);
  }
  let req = api
    .ctx
    .http
    .post(format!("{JOIN}/api/v1/position/searchPosition"))
    .header("origin", JOIN)
    .header("referer", format!("{JOIN}/post.html"))
    .json(&body);
  let v = join_answer(api.json(stamp(req), JOIN).await?)?;
  let rows = v.list("positionList");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(parse::campus_row)
    .map(|j| j.post(false))
    .collect();
  let more = !rows.is_empty() && page * PAGE < v.u64("count").unwrap_or(0);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// A `GET` of careers.tencent.com, its `Data`.
async fn careers_get(
  api: &Api,
  path: &str,
  params: &[(&str, String)],
  referer: &str,
) -> Result<Value> {
  let req = api
    .ctx
    .http
    .get(format!("{CAREERS}/tencentcareer/api{path}"))
    .query("timestamp", Timestamp::now().as_millisecond())
    .queries(params.iter().map(|(k, v)| (*k, v)))
    .header("referer", referer);
  let v = api.json(req, CAREERS).await?;
  match v.i64("Code") {
    Some(200) => Ok(v.at("Data").clone()),
    code => Err(api::upstream(
      "careers.tencent.com",
      v.str("Data"),
      code.unwrap_or_default(),
    )),
  }
}

async fn experienced(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  for (set, what) in [
    (&filter.city, "city"),
    (&filter.batch, "project"),
    (&filter.category, "category"),
  ] {
    if set.is_some() {
      return Err(jobs::unfiltered(
        site,
        &format!("{what} for experienced hires"),
      ));
    }
  }
  let page = req.number_or(1);
  let params = [
    ("countryId", String::new()),
    ("cityId", String::new()),
    ("bgIds", String::new()),
    ("productId", String::new()),
    ("categoryId", String::new()),
    ("parentCategoryId", String::new()),
    ("attrId", String::new()),
    ("keyword", filter.words.clone()),
    ("pageIndex", page.to_string()),
    ("pageSize", PAGE.to_string()),
    ("language", "zh-cn".into()),
    ("area", "cn".into()),
  ];
  let v = careers_get(
    api,
    "/post/Query",
    &params,
    &format!("{CAREERS}/search.html"),
  )
  .await?;
  let rows = v.list("Posts");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(parse::social)
    .map(|j| j.post(false))
    .collect();
  let more = !rows.is_empty() && page * PAGE < v.u64("Count").unwrap_or(0);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  if social(site, filter.kind) {
    experienced(api, site, filter, req).await
  } else {
    campus(api, site, filter, req).await
  }
}

pub async fn read(api: &Api, site: &Site, id: &str) -> Result<Post> {
  if social(site, None) {
    let params = [("postId", id.to_owned()), ("language", "zh-cn".to_owned())];
    let v = careers_get(api, "/post/ByPostId", &params, &parse::social_url(id)).await?;
    return parse::social(&v)
      .map(|j| j.post(true))
      .ok_or_else(|| gone(&parse::social_url(id)));
  }
  let v = join_get(
    api,
    "/api/v1/jobDetails/getJobDetailsByPostId",
    &[("postId", id.to_owned())],
  )
  .await?;
  parse::campus_detail(&v)
    .map(|j| j.post(true))
    .ok_or_else(|| gone(&parse::campus_url(id)))
}

fn gone(url: &str) -> Error {
  Error::not_found(format!(
    "Tencent shows no job at {url} (it may have been taken down)"
  ))
}

/// The projects, cities and categories of join.qq.com.
pub async fn codes(
  api: &Api,
  site: &Site,
  codes: Codes,
  kind: Option<Kind>,
) -> Result<Vec<Collection>> {
  if social(site, kind) {
    return Err(jobs::no_codes(
      &Site {
        kind: Some(Kind::Social),
        ..site.clone()
      },
      codes,
    ));
  }
  Ok(match codes {
    Codes::Batches => parse::batches(&kept(api, "/api/v1/position/getProjectMapping").await?),
    Codes::Cities => parse::cities(&kept(api, "/api/v1/position/getPositionWorkCities").await?),
    Codes::Categories => parse::categories(&kept(api, "/api/v1/position/getPositionFamily").await?),
  })
}
