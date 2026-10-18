//! Jobs: the site's search page (`/web/geek/jobs`), whose own request
//! for its list ATP reads (15 jobs a page; the recommendations of the
//! city when there are no search words), later pages asked from that
//! page as its infinite scroll asks for them (a login is needed: the site
//! shows guests the first page only), and a job's page
//! ([`crate::pages`]).

use atp_core::text::encode_uri_component;
use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt};

use crate::api::{Api, Call, WWW};
use crate::filters::{Code, JobType, NATIONWIDE};
use crate::refs::{self, JobRef};
use crate::{pages, parse};

/// Jobs per page, as the search page asks for them.
const PAGE: u64 = 15;
/// The search's request, for the pages after the first.
const SEARCH: &str = "/wapi/zpgeek/search/joblist.json";

/// What a search is narrowed to, in the site's codes.
#[derive(Debug, Clone)]
pub struct Filter {
  pub words: String,
  pub city: String,
  pub salary: Option<&'static str>,
  pub experience: Option<&'static str>,
  pub degree: Option<&'static str>,
  pub industry: Option<String>,
  pub size: Option<&'static str>,
  pub stage: Option<&'static str>,
  pub job_type: Option<&'static str>,
}

impl Default for Filter {
  fn default() -> Self {
    Self {
      words: String::new(),
      city: NATIONWIDE.into(),
      salary: None,
      experience: None,
      degree: None,
      industry: None,
      size: None,
      stage: None,
      job_type: None,
    }
  }
}

/// `search WORDS`: all of China; `--filter` keeps full-time or part-time
/// jobs.
pub fn search_filter(query: &Query) -> Filter {
  Filter {
    words: query.keyword.clone(),
    job_type: query
      .filter
      .as_deref()
      .and_then(JobType::from_name)
      .map(Code::code),
    ..Filter::default()
  }
}

/// The filters under the names the search page reads from its address
/// and sends in its requests.
fn params(filter: &Filter) -> Vec<(&'static str, String)> {
  let mut params = vec![("city", filter.city.clone())];
  if !filter.words.is_empty() {
    params.insert(0, ("query", filter.words.clone()));
  }
  let codes = [
    ("experience", filter.experience),
    ("degree", filter.degree),
    ("salary", filter.salary),
    ("industry", filter.industry.as_deref()),
    ("scale", filter.size),
    ("stage", filter.stage),
    ("jobType", filter.job_type),
  ];
  for (key, code) in codes {
    if let Some(code) = code {
      params.push((key, code.to_owned()));
    }
  }
  params
}

/// The search page of a filter.
pub fn search_page(filter: &Filter) -> String {
  let query: Vec<String> = params(filter)
    .iter()
    .map(|(k, v)| format!("{k}={}", encode_uri_component(v)))
    .collect();
  format!("{WWW}/web/geek/jobs?{}", query.join("&"))
}

/// Whether the page shows search results (rather than recommendations).
fn searching(filter: &Filter) -> bool {
  !filter.words.is_empty() || filter.stage.is_some()
}

/// One page: the first as the search page loads it, the next ones (with
/// a login) as its scroll asks for them.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let from = search_page(filter);
  let v = if page == 1 {
    api.jobs(&from).await?
  } else {
    let mut form = vec![("page", page.to_string()), ("pageSize", PAGE.to_string())];
    form.extend(params(filter));
    form.extend([
      ("scene", "1".to_owned()),
      ("encryptExpectId", String::new()),
    ]);
    let call = Call {
      path: SEARCH,
      query: &[],
      form: Some(&form),
      from: &from,
    };
    api.in_tab(&call).await?
  };
  let items: Vec<Post> = v.list("jobList").iter().filter_map(parse::card).collect();
  let more =
    api.logged_in() && searching(filter) && !items.is_empty() && v.bool("hasMore") == Some(true);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// A job's page as ATP reads it, and the job as an argument.
pub struct Job {
  pub post: Post,
  pub job: JobRef,
}

/// The `securityId` and `lid` of the 立即沟通 link a page shows.
fn ids(page: &Value, job: &mut JobRef) {
  let Some(start) = page.str("start").filter(|s| !s.is_empty()) else {
    return;
  };
  let link = format!(
    "{WWW}/job_detail/{}.html?{}",
    job.id,
    start.split_once('?').map_or("", |(_, q)| q)
  );
  // The page's ids are fresher than those of a link kept from earlier.
  if let Ok(found) = refs::job(&link) {
    job.security_id = found.security_id.or(job.security_id.take());
    job.lid = found.lid.or(job.lid.take());
  }
}

/// A job's page, with the ids actions on it need.
pub async fn job(api: &Api, arg: &str) -> Result<Job> {
  let mut job = refs::job(arg)?;
  let url = job.url();
  let page = pages::job(api, &url).await?;
  ids(&page, &mut job);
  let post = parse::job_page(&page, &job).ok_or_else(|| {
    Error::upstream(format!(
      "BOSS Zhipin's job page {url} came without the job ATP looks for (the page may have changed)"
    ))
  })?;
  Ok(Job { post, job })
}

/// A job's page: the whole description, pay, place, experience, degree,
/// skills, benefits, the address and the company.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  Ok(job(api, arg).await?.post)
}
