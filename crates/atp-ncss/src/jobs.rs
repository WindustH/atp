//! Jobs: the search of the 职位信息 page (`/student/jobs/jobslist/ajax/`),
//! the jobs of a job fair (`/student/m/api/jobfair/jobs`), a company's jobs
//! (`/student/jobs/corp/jobs.json`) and one job's page.
//!
//! Lists answer pages of `data.list` with `data.pagenation` (`count` items,
//! `total` pages, `offset` the page). The search shows ten pages without a
//! login; further pages want one.

use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::filters::JobType;
use crate::{parse, refs};

/// Cards per page: the site's pages ask for 10; 20 is taken as well.
const PAGE: u64 = 20;
/// The page the job lists are loaded from.
const JOBS_PAGE: &str = "/student/jobs/index.html";

/// What a job search is narrowed to, in the site's codes.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  pub words: String,
  pub area: String,
  pub job_type: Option<JobType>,
  pub kind: String,
  pub degree: String,
  pub salary: String,
  pub category: String,
  pub industries: Vec<String>,
  pub property: String,
  pub key_field: bool,
  pub featured: bool,
  /// `sourcesType` and `sourcesName`.
  pub source: (String, String),
}

impl Filter {
  /// The query of the search, every key as the page sends it.
  fn params(&self, page: u64) -> Vec<(&'static str, String)> {
    let industries: String = self.industries.iter().map(|i| format!("{i},")).collect();
    vec![
      (
        "jobType",
        self.job_type.map_or("", JobType::code).to_owned(),
      ),
      ("areaCode", self.area.clone()),
      ("jobName", self.words.clone()),
      ("monthPay", self.salary.clone()),
      ("industrySectors", industries),
      ("property", self.property.clone()),
      ("categoryCode", self.category.clone()),
      (
        "memberLevel",
        if self.featured { "2" } else { "" }.to_owned(),
      ),
      ("recruitType", self.kind.clone()),
      ("offset", page.to_string()),
      ("limit", PAGE.to_string()),
      ("keyUnits", if self.key_field { "1" } else { "" }.to_owned()),
      ("degreeCode", self.degree.clone()),
      ("sourcesName", self.source.1.clone()),
      ("sourcesType", self.source.0.clone()),
    ]
  }
}

/// `search WORDS`: every job of the site; `--filter` keeps one job type or
/// the hiring announcements.
pub fn search_filter(query: &Query) -> Filter {
  let mut filter = Filter {
    words: query.keyword.clone(),
    ..Filter::default()
  };
  match query.filter.as_deref() {
    Some("announcement") => filter.kind = "1".into(),
    Some(other) => filter.job_type = JobType::from_name(other),
    None => {}
  }
  filter
}

/// The next page after `page` of a `pagenation`, while there are more.
fn next(v: &Value, page: u64, items: usize) -> Option<String> {
  let pages = v.u64("pagenation.total").unwrap_or(0);
  (items > 0 && page < pages).then(|| (page + 1).to_string())
}

/// The cards of a list; `job_type` filled in when the filter names it.
fn cards(v: &Value, job_type: Option<JobType>) -> Vec<Post> {
  v.list("list")
    .iter()
    .filter_map(parse::card)
    .map(|mut post| {
      if let Some(kind) = job_type {
        post
          .extra
          .entry("job_type".to_owned())
          .or_insert_with(|| json!(kind.name()));
      }
      post
    })
    .collect()
}

/// One page of the job search.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let v = api
    .data(
      "/student/jobs/jobslist/ajax/",
      &filter.params(page),
      JOBS_PAGE,
    )
    .await?;
  let items = cards(&v, filter.job_type);
  let cursor = next(&v, page, items.len());
  Ok(Page::new(items, cursor))
}

/// One page of a job fair's jobs, with the filters its page has.
pub async fn fair(api: &Api, fair: &str, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let mut params: Vec<(&str, String)> = filter
    .params(page)
    .into_iter()
    .filter(|(k, _)| {
      [
        "jobType",
        "areaCode",
        "jobName",
        "monthPay",
        "industrySectors",
        "property",
        "categoryCode",
        "offset",
        "limit",
      ]
      .contains(k)
    })
    .collect();
  params.insert(0, ("fairId", fair.to_owned()));
  let referer = format!("/student/m/jobfair/jobs.html?fairId={fair}");
  let v = api
    .data("/student/m/api/jobfair/jobs", &params, &referer)
    .await?;
  let items = cards(&v, filter.job_type);
  let cursor = next(&v, page, items.len());
  Ok(Page::new(items, cursor))
}

/// One page of a company's open jobs, as its page lists them.
pub async fn company(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let id = refs::company(arg)?;
  let page = req.number_or(1);
  let params = [
    ("corpId", id.clone()),
    ("offset", page.to_string()),
    ("limit", PAGE.to_string()),
  ];
  let referer = format!("/student/jobs/{id}/corp.html");
  let v = api
    .data("/student/jobs/corp/jobs.json", &params, &referer)
    .await?;
  let items = cards(&v, None);
  let cursor = next(&v, page, items.len());
  Ok(Page::new(items, cursor))
}

/// A job's page as HTML, or `not_found` when the site shows none.
pub async fn page(api: &Api, id: &str) -> Result<String> {
  api.page(&format!("/student/jobs/{id}/detail.html")).await
}

/// A job's page: the whole description, pay, degree, openings, majors,
/// place, where it was posted and the company.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::job(arg)?;
  let html = page(api, &id).await?;
  parse::job(&html, &id)
    .map(|page| page.post)
    .ok_or_else(|| gone(&id))
}

/// The error of a job the site no longer shows.
pub fn gone(id: &str) -> Error {
  Error::not_found(format!(
    "NCSS shows no job at {} (it may have been taken down)",
    refs::job_url(id)
  ))
}
