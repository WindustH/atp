//! Kuaishou's campus site (campus.kuaishou.cn), through the requests its
//! pages make under `/recruit/campus/e/api/v1`: the projects
//! (`/open/sub-project/list`: 2027应届生, 2027实习生 …), the search
//! (`POST /open/positions/simple`: projects by code, words, cities and
//! categories by code, page), a job (`/open/positions/find?id=`) and the
//! names behind the codes (`/dictionary/batch`: cities, categories).
//! Answers are `{code: 0, message, result}`. Experienced hires live on
//! another site (zhaopin.kuaishou.cn), which ATP does not read.

use std::time::Duration;

use atp_core::{Collection, Error, Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::{self, Api};
use crate::jobs::{self, Codes, Filter, Kind};
use crate::parse::kuaishou as parse;
use crate::sites::Site;

pub const WWW: &str = "https://campus.kuaishou.cn";
const BASE: &str = "https://campus.kuaishou.cn/recruit/campus/e";
/// Jobs per page.
const PAGE: u64 = 20;
/// How long projects and dictionaries are kept.
const DAY: Duration = Duration::from_secs(24 * 3600);

fn answer(v: Value) -> Result<Value> {
  match v.i64("code") {
    Some(0) => Ok(v.at("result").clone()),
    code => Err(api::upstream(
      "campus.kuaishou.cn",
      v.str("message"),
      code.unwrap_or_default(),
    )),
  }
}

async fn get(api: &Api, path: &str, params: &[(&str, String)]) -> Result<Value> {
  let req = api
    .ctx
    .http
    .get(format!("{BASE}/api/v1{path}"))
    .queries(params.iter().map(|(k, v)| (*k, v)))
    .header("referer", format!("{BASE}/"));
  answer(api.json(req, &format!("{BASE}/")).await?)
}

/// What the site keeps a day: projects and dictionaries.
async fn kept(api: &Api, key: &str, path: &str, params: &[(&str, String)]) -> Result<Value> {
  let key = format!("kuaishou-{key}");
  if let Some(v) = api.ctx.store.cache_get::<Value>(&key, DAY) {
    return Ok(v);
  }
  let v = get(api, path, params).await?;
  api.ctx.store.cache_put(&key, &v);
  Ok(v)
}

async fn projects(api: &Api) -> Result<Value> {
  let params = [("pageNum", "1".to_owned()), ("pageSize", "50".to_owned())];
  kept(api, "projects", "/open/sub-project/list", &params).await
}

async fn dictionary(api: &Api) -> Result<Value> {
  let params = [(
    "types",
    "workLocation,positionCategory,positionNature".to_owned(),
  )];
  kept(api, "dictionary", "/dictionary/batch", &params).await
}

/// What names a job's codes; a dictionary that cannot be read leaves the
/// codes as they are.
async fn known(api: &Api) -> parse::Names {
  parse::Names {
    dict: dictionary(api).await.unwrap_or_default(),
    projects: projects(api).await.unwrap_or_default(),
  }
}

/// The projects a search covers: the one asked for, else the newest of
/// the kind (both kinds by default).
async fn covered(api: &Api, filter: &Filter, kind: Option<Kind>) -> Result<Vec<String>> {
  if let Some(batch) = &filter.batch {
    return Ok(vec![batch.trim().to_owned()]);
  }
  let v = projects(api).await?;
  let kinds: &[&str] = match kind {
    Some(Kind::Campus) => &["fulltime"],
    Some(Kind::Intern) => &["intern"],
    _ => &["fulltime", "intern"],
  };
  let codes: Vec<String> = kinds
    .iter()
    .filter_map(|kind| parse::newest(&v, kind))
    .collect();
  if codes.is_empty() {
    return Err(Error::not_found(
      "campus.kuaishou.cn lists no project open now",
    ));
  }
  Ok(codes)
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let kind = filter.kind(site);
  if kind == Some(Kind::Social) {
    return Err(Error::input(format!(
      "{} lists campus jobs and internships; Kuaishou's experienced hires are on zhaopin.kuaishou.cn, which ATP does not read",
      site.host()
    )));
  }
  let page = req.number_or(1);
  let mut body = json!({
    "recruitSubProjectCodes": covered(api, filter, kind).await?,
    "pageSize": PAGE,
    "pageNum": page,
  });
  if !filter.words.is_empty() {
    body["name"] = json!(filter.words);
  }
  if filter.city.is_some() || filter.category.is_some() {
    let dict = dictionary(api).await?;
    if let Some(city) = &filter.city {
      body["workLocationCodes"] = json!([jobs::choose(&parse::cities(&dict), city, site, "city")?]);
    }
    if let Some(category) = &filter.category {
      let chosen = jobs::choose(&parse::categories(&dict), category, site, "category")?;
      body["positionCategoryCodes"] = json!(parse::with_children(&dict, &chosen));
    }
  }
  let req = api
    .ctx
    .http
    .post(format!("{BASE}/api/v1/open/positions/simple"))
    .header("origin", WWW)
    .header("referer", format!("{BASE}/"))
    .json(&body);
  let v = answer(api.json(req, &format!("{BASE}/")).await?)?;
  let known = known(api).await;
  let rows = v.list("list");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(|row| parse::job(row, &known))
    .map(|j| j.post(false))
    .collect();
  let more = !rows.is_empty() && page * PAGE < v.u64("total").unwrap_or(0);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

pub async fn read(api: &Api, id: &str) -> Result<Post> {
  let v = get(api, "/open/positions/find", &[("id", id.to_owned())]).await?;
  let known = known(api).await;
  parse::job(&v, &known).map(|j| j.post(true)).ok_or_else(|| {
    Error::not_found(format!(
      "Kuaishou shows no job at {} (it may have been taken down)",
      parse::job_url(id)
    ))
  })
}

/// The projects, cities and categories.
pub async fn codes(api: &Api, site: &Site, codes: Codes) -> Result<Vec<Collection>> {
  if site.kind == Some(Kind::Social) {
    return Err(jobs::no_codes(site, codes));
  }
  Ok(match codes {
    Codes::Batches => parse::projects(&projects(api).await?),
    Codes::Cities => parse::cities(&dictionary(api).await?),
    Codes::Categories => parse::categories(&dictionary(api).await?),
  })
}
