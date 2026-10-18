//! Jobs: the search of the 校招, 实习 and 社招 pages
//! (`/np-api/u/job/square-search`, 20 cards a page), a job's page
//! (`/jobs/detail/<id>`, rendered with `window.__INITIAL_STATE__`), saving
//! jobs (`/np-api/u/job/mark-job`, `unmark-job`), your saved jobs
//! (`/api/sparta/user/collect/job`) and your applications
//! (`/v2/resume/deliver-list`).

use atp_core::{Action, Error, Page, PageReq, Post, Query, Result, Value, ValueExt, json};

use crate::api::{Api, Body, Route};
use crate::{parse, refs, users};

/// Cards per page, as the site asks for them.
const PAGE: u64 = 20;
/// The source of jobs from companies' own career sites (官网).
const OFFICIAL: i64 = 3030;

/// What a job search is narrowed to, in the site's codes.
#[derive(Debug, Clone)]
pub struct Filter {
  pub query: String,
  /// `recruitType`: 1 校招, 2 实习, 3 社招.
  pub kind: u8,
  pub cities: Vec<String>,
  /// `careerJobId`.
  pub career: Option<String>,
  /// `graduateYearList`: `2027届` …
  pub years: Vec<String>,
  /// Monthly pay in thousands.
  pub salary: Option<(u32, u32)>,
  /// `companyNature`.
  pub nature: Option<u32>,
  /// `scaleList`.
  pub sizes: Vec<u8>,
  /// Newest first instead of the site's recommendation.
  pub newest: bool,
}

impl Default for Filter {
  fn default() -> Self {
    Self {
      query: String::new(),
      kind: 1,
      cities: Vec::new(),
      career: None,
      years: Vec::new(),
      salary: None,
      nature: None,
      sizes: Vec::new(),
      newest: false,
    }
  }
}

/// `search WORDS`: 校招 jobs, or with `--filter intern|social` the others;
/// `--sort new` puts the newest first.
pub fn search_filter(query: &Query) -> Result<Filter> {
  let kind = match query.filter.as_deref() {
    Some("intern") => 2,
    Some("social") => 3,
    _ => 1,
  };
  if query.sort.as_deref() == Some("hot") {
    return Err(Error::input(
      "jobs come in the site's order or the newest first (`--sort new`); `--sort hot` is for posts (`--filter posts`)",
    ));
  }
  Ok(Filter {
    query: query.keyword.clone(),
    kind,
    newest: query.sort.as_deref() == Some("new"),
    ..Filter::default()
  })
}

/// The search form as the job pages send it; lists as repeated keys.
fn form(filter: &Filter, page: u64) -> Vec<(&'static str, String)> {
  let mut form = vec![
    ("requestFrom", "1".to_owned()),
    ("page", page.to_string()),
    ("pageSize", PAGE.to_string()),
    ("recruitType", filter.kind.to_string()),
    ("order", if filter.newest { "1" } else { "0" }.to_owned()),
    ("pageSource", "5001".to_owned()),
  ];
  if !filter.query.is_empty() {
    form.push(("query", filter.query.clone()));
  }
  form.extend(filter.cities.iter().map(|c| ("jobCity", c.clone())));
  form.extend(filter.career.iter().map(|c| ("careerJobId", c.clone())));
  form.extend(filter.years.iter().map(|y| ("graduateYearList", y.clone())));
  if let Some((min, max)) = filter.salary {
    form.extend([
      ("salaryMin", min.to_string()),
      ("salaryMax", max.to_string()),
      ("salaryType", "2".to_owned()),
    ]);
  }
  form.extend(filter.nature.map(|n| ("companyNature", n.to_string())));
  form.extend(filter.sizes.iter().map(|s| ("scaleList", s.to_string())));
  form
}

/// The next page number of a `{currentPage, totalPage}` answer.
fn next(v: &Value, page: u64, items: usize) -> Option<String> {
  let current = v.u64("currentPage").unwrap_or(page);
  let pages = v.u64("totalPage").unwrap_or(0);
  (items > 0 && current < pages).then(|| (current + 1).to_string())
}

/// One page of the job search. Cards come wrapped with their source
/// (`rc_type`): jobs posted on the site, or jobs of companies' own career
/// sites (官网, 3030), which have no page here and are applied to through
/// the site's 网申助手; those are left out.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let v = api
    .form(Route::Jobs, "/u/job/square-search", &form(filter, page))
    .await?;
  let cards = v.list("datas");
  let items: Vec<Post> = cards
    .iter()
    .filter_map(|card| match card.i64("rc_type") {
      Some(OFFICIAL) => None,
      Some(_) => parse::job(card.at("data"), None),
      None => parse::job(card, None),
    })
    .collect();
  Ok(Page::new(items, next(&v, page, cards.len())))
}

/// The data a server-rendered page was built from
/// (`window.__INITIAL_STATE__ = {…}`).
fn initial_state(html: &str) -> Option<Value> {
  let start = html.find("window.__INITIAL_STATE__")?;
  let rest = &html[start..];
  let json = rest[rest.find('=')? + 1..].trim_start();
  serde_json::Deserializer::from_str(json)
    .into_iter::<Value>()
    .next()?
    .ok()
}

/// A job's page: the whole description, pay, cities, address, degree,
/// deadline, the company with its industry and size. The page carries
/// them; when it comes without them (it is sometimes rendered empty), the
/// page's own calls give them (`/np-api/u/job/detail`, `job-company`).
pub async fn read(api: &Api, id: &str) -> Result<Post> {
  let html = api.page(&format!("/jobs/detail/{id}")).await?;
  let gone = || {
    Error::not_found(format!(
      "Nowcoder shows no job at {} (it may have been taken down)",
      refs::job_url(id)
    ))
  };
  let state = initial_state(&html).unwrap_or_default();
  let page = state.at("store.jobDetail");
  if page.at("detail").str("id").is_some() {
    return parse::job(page.at("detail"), Some(page.at("jobCompany"))).ok_or_else(gone);
  }
  let params = [("id", id.to_owned())];
  let job = api.get(Route::Jobs, "/u/job/detail", &params).await?;
  if job.str("id").is_none() {
    return Err(gone());
  }
  let company = api
    .get(Route::Jobs, "/u/job/job-company", &params)
    .await
    .unwrap_or_default();
  parse::job(&job, Some(&company)).ok_or_else(gone)
}

/// Save a job (收藏) or take it out.
pub async fn favorite(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::job(arg)?;
  let (path, key) = if undo {
    ("/u/job/unmark-job", "jobIds")
  } else {
    ("/u/job/mark-job", "jobId")
  };
  let params = [(key, id.clone()), ("jobSource", "nowpick".to_owned())];
  api.write(Route::Jobs, path, &params, Body::None).await?;
  let action = if undo { "unfavorite" } else { "favorite" };
  Ok(Action::done(action, &id).with_url(refs::job_url(&id)))
}

/// Your saved jobs, as your profile's 收藏 tab lists them.
pub async fn favorites(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let uid = users::uid(api).await?;
  let page = req.number_or(1);
  let body = json!({
    "pageNo": page,
    "pageSize": PAGE,
    "userId": uid.parse::<u64>().unwrap_or_default(),
  });
  let v = api
    .json(Route::Gateway, "/api/sparta/user/collect/job", &body)
    .await?;
  let items: Vec<Post> = v
    .list("records")
    .iter()
    .filter_map(parse::saved_job)
    .collect();
  let current = v.u64("current").unwrap_or(page);
  let more = v.u64("totalPage").is_some_and(|pages| current < pages);
  let cursor = (more && !items.is_empty()).then(|| (current + 1).to_string());
  Ok(Page::new(items, cursor))
}

/// Your applications (投递记录), by the stage tabs of the site's list
/// (`stage`: 0 all, 7 viewed, 1 past the first screening, 2 turned down).
pub async fn applied(api: &Api, stage: i64, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let params = [("page", page.to_string()), ("stage", stage.to_string())];
  let v = api
    .own(Route::Site, "/v2/resume/deliver-list", &params)
    .await?;
  let items: Vec<Post> = v
    .list("deliverData")
    .iter()
    .map(parse::application)
    .collect();
  // The page size is the site's; full pages so far reach `page × items`.
  let total = v.u64("pageInfo.totalCount");
  let reached = page * items.len() as u64;
  let more = !items.is_empty() && total.is_none_or(|total| reached < total);
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}
