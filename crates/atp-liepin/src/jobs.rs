//! Jobs: the search of the site's 职位 page
//! (`searchfront4c.pc-search-job`, 40 cards a page, pages counted from
//! 0), a company's jobs (`searchfront4c.pc-comp-homepage-search-job`, 30
//! a page) and one job's page.

use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt, json};

use crate::api::{Api, search_id};
use crate::filters::NATIONWIDE;
use crate::{parse, refs};

/// Cards per search page, as the site asks for them.
const SEARCH_PAGE: u64 = 40;
/// Jobs per page of a company's list.
const COMPANY_PAGE: u64 = 30;

/// What a search is narrowed to: the codes of the site's filters.
#[derive(Debug, Clone)]
pub struct Filter {
  pub key: String,
  /// City (`dq`); 410 is all of China.
  pub city: String,
  pub salary: String,
  pub experience: String,
  pub degree: String,
  pub industry: String,
  pub size: String,
  pub stage: String,
  /// 1 headhunter jobs, 2 company jobs.
  pub kind: String,
}

impl Default for Filter {
  fn default() -> Self {
    Self {
      key: String::new(),
      city: NATIONWIDE.into(),
      salary: String::new(),
      experience: String::new(),
      degree: String::new(),
      industry: String::new(),
      size: String::new(),
      stage: String::new(),
      kind: String::new(),
    }
  }
}

/// The next page number after `page`, while there are more.
fn next(page: u64, v: &Value, items: usize) -> Option<String> {
  let pages = v.u64("pagination.totalPage").unwrap_or(0);
  (items > 0 && page + 1 < pages).then(|| (page + 1).to_string())
}

/// `search WORDS`: all of China; `--filter` keeps headhunter or company jobs.
pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<Post>> {
  let kind = match query.filter.as_deref() {
    Some("headhunter") => "1",
    Some("company") => "2",
    _ => "",
  };
  let filter = Filter {
    key: query.keyword.clone(),
    kind: kind.into(),
    ..Filter::default()
  };
  list(api, &filter, req).await
}

/// One page of the site's job search.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(0);
  let body = json!({
    "mainSearchPcConditionForm": {
      "city": filter.city,
      "dq": filter.city,
      "pubTime": "",
      "currentPage": page,
      "pageSize": SEARCH_PAGE,
      "key": filter.key,
      "suggestTag": "",
      "workYearCode": filter.experience,
      "compId": "",
      "compName": "",
      "compTag": "",
      "industry": filter.industry,
      "salaryCode": filter.salary,
      "jobKind": filter.kind,
      "compScale": filter.size,
      "compKind": "",
      "compStage": filter.stage,
      "eduLevel": filter.degree,
    },
    "passThroughForm": {
      "scene": if page == 0 { "init" } else { "page" },
      "skId": "",
      "fkId": "",
      "ckId": search_id(),
      "suggest": null,
    },
  });
  let v = api.json("searchfront4c.pc-search-job", body).await?;
  let items: Vec<Post> = v
    .list("data.jobCardList")
    .iter()
    .filter(|card| !card.at("job").is_null())
    .map(parse::card)
    .collect();
  let cursor = next(page, &v, items.len());
  Ok(Page::new(items, cursor))
}

/// The jobs a company has open, as its 职位 tab lists them.
pub async fn company(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let id = refs::company(arg)?;
  let comp: u64 = id
    .parse()
    .map_err(|_| Error::input(format!("not a company number: {id}")))?;
  let page = req.number_or(0);
  let body = json!({
    "compJobSearchCondition": {
      "compId": comp,
      "dq": "",
      "jobTitleCode": "",
      "pageSize": COMPANY_PAGE,
      "curPage": page,
    },
    "passThroughForm": {
      "scene": if page == 0 { "init" } else { "page" },
      "ckId": search_id(),
    },
  });
  let v = api
    .json("searchfront4c.pc-comp-homepage-search-job", body)
    .await?;
  let items: Vec<Post> = v
    .list("data")
    .iter()
    .map(|card| {
      let mut post = parse::card(card);
      post.extra.insert("company_id".into(), json!(id));
      post
    })
    .collect();
  let cursor = next(page, &v, items.len());
  Ok(Page::new(items, cursor))
}

/// A job read from its page, with the ids the site's actions take.
pub struct Job {
  pub post: Post,
  pub url: String,
  /// `jobId` and `jobKind` of the page's `$CONFIG`.
  pub id: String,
  pub kind: String,
}

pub async fn job(api: &Api, arg: &str) -> Result<Job> {
  let url = refs::job(arg)?.url();
  let html = api.page(&url).await?;
  let gone = || {
    Error::not_found(format!(
      "Liepin shows no job at {url} (it may have been taken down)"
    ))
  };
  let config = parse::config(&html).ok_or_else(gone)?;
  let post = parse::job(&html, &url).ok_or_else(gone)?;
  Ok(Job {
    id: config.str("jobId").ok_or_else(gone)?,
    kind: config.str("jobKind").unwrap_or_else(|| "2".into()),
    post,
    url,
  })
}

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  Ok(job(api, arg).await?.post)
}
