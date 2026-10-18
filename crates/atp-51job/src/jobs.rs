//! Jobs: the search of we.51job.com's 搜索 page (`api/job/search-pc`, 20 a
//! page, each card with the job's whole description), asked from that page
//! in the browser tab, and a job's page on jobs.51job.com
//! ([`crate::pages`]).

use atp_core::text::encode_uri_component;
use atp_core::{Error, Page, PageReq, Post, Query, Result, ValueExt};

use crate::api::{Api, WE};
use crate::filters::{self, Code, JobType, NATIONWIDE};
use crate::{pages, parse, refs};

/// Cards per page, as the search page asks for them.
const PAGE: u64 = 20;

/// What a search is narrowed to, in the site's codes.
#[derive(Debug, Clone)]
pub struct Filter {
  pub words: String,
  pub city: String,
  pub salary: Option<&'static str>,
  pub experience: Option<&'static str>,
  pub degree: Option<&'static str>,
  pub company_type: Option<&'static str>,
  pub size: Option<&'static str>,
  pub job_type: Option<&'static str>,
  pub sort: &'static str,
}

impl Default for Filter {
  fn default() -> Self {
    Self {
      words: String::new(),
      city: NATIONWIDE.into(),
      salary: None,
      experience: None,
      degree: None,
      company_type: None,
      size: None,
      job_type: None,
      sort: filters::sort(None),
    }
  }
}

/// `search WORDS`: all of China; `--filter` keeps one kind of job.
pub fn search_filter(query: &Query) -> Filter {
  Filter {
    words: query.keyword.clone(),
    job_type: query
      .filter
      .as_deref()
      .and_then(JobType::from_name)
      .map(Code::code),
    sort: filters::sort(query.sort.as_deref()),
    ..Filter::default()
  }
}

/// The search page of the words, which the search's requests come from.
fn search_page(filter: &Filter) -> String {
  format!(
    "{WE}/pc/search?keyword={}&searchType=2",
    encode_uri_component(&filter.words)
  )
}

/// The search's query, in the order and with the fields the search page
/// sends.
fn query(filter: &Filter, page: u64) -> Vec<(&'static str, String)> {
  let code = |value: Option<&str>| value.unwrap_or_default().to_owned();
  vec![
    ("keyword", filter.words.clone()),
    ("searchType", "2".into()),
    ("function", String::new()),
    ("industry", String::new()),
    ("jobArea", filter.city.clone()),
    ("jobArea2", String::new()),
    ("landmark", String::new()),
    ("metro", String::new()),
    ("salary", code(filter.salary)),
    ("workYear", code(filter.experience)),
    ("degree", code(filter.degree)),
    ("companyType", code(filter.company_type)),
    ("companySize", code(filter.size)),
    ("jobType", code(filter.job_type)),
    ("issueDate", String::new()),
    ("sortType", filter.sort.into()),
    ("pageNum", page.to_string()),
    ("requestId", String::new()),
    ("pageSize", PAGE.to_string()),
    ("source", "1".into()),
    ("accountId", String::new()),
    ("pageCode", "sou|sou|soulb".into()),
    ("scene", "7".into()),
  ]
}

/// One page of the search.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let v = api
    .we(
      "api/job/search-pc",
      &query(filter, page),
      &search_page(filter),
    )
    .await?;
  let job = v.at("job");
  let items: Vec<Post> = job.list("items").iter().filter_map(parse::card).collect();
  let total = job
    .u64("totalCount")
    .or_else(|| job.u64("totalcount"))
    .unwrap_or_default();
  let more = !items.is_empty() && page * PAGE < total;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// A job's page: the whole description, pay, place, experience, degree,
/// benefits and the company.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let (number, url) = refs::job(arg)?;
  let page = pages::job(api, &url).await?;
  parse::job_page(&page, &number, &url).ok_or_else(|| {
    Error::upstream(format!(
      "51job's job page {url} came without the job ATP looks for (the page may have changed)"
    ))
  })
}
