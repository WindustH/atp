//! Meituan's career site (zhaopin.meituan.com), through the requests its
//! pages make: `POST /api/official/job/getJobList` (words, kinds of
//! hiring as `jobType` codes, cities by code, page),
//! `/api/official/job/getJobDetail` (a job) and `/api/official/city/search`
//! (a city's code by its name). Answers are `{status: 1, message, data}`.
//! The pages sign none of these.

use atp_core::{Error, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::{self, Api};
use crate::jobs::{self, Filter, Kind};
use crate::parse::meituan as parse;
use crate::sites::Site;

pub const WWW: &str = "https://zhaopin.meituan.com";
/// Jobs per page.
const PAGE: u64 = 20;

/// A call of the site, its `data`.
async fn call(api: &Api, path: &str, body: &Value, referer: &str) -> Result<Value> {
  let req = api
    .ctx
    .http
    .post(format!("{WWW}/api/official{path}"))
    .header("origin", WWW)
    .header("referer", referer)
    .json(body);
  let v = api.json(req, referer).await?;
  match v.i64("status") {
    Some(1) => Ok(v.at("data").clone()),
    code => Err(api::upstream(
      "zhaopin.meituan.com",
      v.str("message"),
      code.unwrap_or_default(),
    )),
  }
}

/// `jobType` of a kind: 1 应届, 2 实习, 3 社招.
fn code(kind: Kind) -> &'static str {
  match kind {
    Kind::Campus => "1",
    Kind::Intern => "2",
    Kind::Social => "3",
  }
}

/// A city's code, by its name.
async fn city(api: &Api, site: &Site, name: &str) -> Result<String> {
  if name.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(name.to_owned());
  }
  let v = call(
    api,
    "/city/search",
    &json!({"keyword": name.trim()}),
    &format!("{WWW}/web/position"),
  )
  .await?;
  v.as_array()
    .and_then(|list| list.first())
    .and_then(|c| c.str("code"))
    .ok_or_else(|| Error::input(format!("{} knows no city {name}", site.host())))
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  if filter.batch.is_some() || filter.category.is_some() {
    return Err(jobs::unfiltered(site, "batch or category"));
  }
  let kinds = match filter.kind(site) {
    Some(kind) => vec![kind],
    None => vec![Kind::Campus, Kind::Intern],
  };
  let cities = match &filter.city {
    Some(name) => vec![json!({"code": city(api, site, name).await?})],
    None => vec![],
  };
  let page = req.number_or(1);
  let body = json!({
    "page": {"pageNo": page, "pageSize": PAGE},
    "jobShareType": "1",
    "keywords": filter.words,
    "cityList": cities,
    "department": [],
    "jfJgList": [],
    "jobType": kinds.iter().map(|k| json!({"code": code(*k), "subCode": []})).collect::<Vec<_>>(),
    "typeCode": [],
    "specialCode": [],
  });
  let referer = format!("{WWW}/web/position");
  let v = call(api, "/job/getJobList", &body, &referer).await?;
  let rows = v.list("list");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(parse::job)
    .map(|j| j.post(false))
    .collect();
  let more = !rows.is_empty() && page * PAGE < v.u64("page.totalCount").unwrap_or(0);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

pub async fn read(api: &Api, _site: &Site, id: &str) -> Result<Post> {
  let body = json!({"jobUnionId": id, "jobShareType": "1"});
  let v = call(api, "/job/getJobDetail", &body, &parse::job_url(id, None)).await?;
  parse::job(&v).map(|j| j.post(true)).ok_or_else(|| {
    Error::not_found(format!(
      "Meituan shows no job at {} (it may have been taken down)",
      parse::job_url(id, None)
    ))
  })
}
