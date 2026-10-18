//! Jobs: LinkedIn's job search (`seeMoreJobPostings/search`, 10 cards a
//! page from the offset `start`, up to its 1000th result), a company's
//! open jobs (the same search by company, worldwide) and one job's page
//! (`jobPosting/<id>`).

use atp_core::output::note;
use atp_core::{Error, Page, PageReq, Post, Query, Result};

use crate::api::{Api, GUEST, unexpected};
use crate::filters::Filter;
use crate::{parse, refs, users};

/// Cards a page of the search holds.
const PAGE: u64 = 10;
/// How far LinkedIn pages a search: no results from this offset on.
const CAP: u64 = 1000;
/// LinkedIn's place standing for the whole world (`geoId`).
const WORLDWIDE: &str = "92000000";

/// `search WORDS`, wherever LinkedIn takes the visitor to be.
pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<Post>> {
  list(api, &Filter::words(&query.keyword), req).await
}

/// One page of the search.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let start = req.number_or(0);
  if start >= CAP {
    return Err(Error::input(capped()).with_hint(NARROW));
  }
  let mut query = filter.query();
  query.push(("start", start.to_string()));
  let url = format!("{GUEST}/seeMoreJobPostings/search");
  let Some(html) = api.html(&url, &query).await? else {
    return Err(Error::upstream("LinkedIn's job search answered 404"));
  };
  let items = parse::cards::cards(&html);
  if items.is_empty() && parse::cards::any(&html) {
    return Err(unexpected("job cards ATP can read", &html));
  }
  let next = start + PAGE;
  let full = items.len() as u64 >= PAGE;
  if full && next >= CAP {
    note(&format!("{}; {NARROW}", capped()));
  }
  let cursor = (full && next < CAP).then(|| next.to_string());
  Ok(Page::new(items, cursor))
}

/// LinkedIn's limit on how far a search goes.
fn capped() -> String {
  format!("LinkedIn shows no more than the first {CAP} jobs of a search")
}

/// How to see jobs beyond it.
const NARROW: &str = "narrow the search (--posted, --location, --company …) to see others";

/// The jobs a company has open, wherever they are.
pub async fn company(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let ids = users::job_ids(api, arg).await?;
  let filter = Filter {
    companies: vec![ids],
    geo: Some(WORLDWIDE.into()),
    ..Filter::default()
  };
  list(api, &filter, req).await
}

/// A job's page.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::job(arg)?;
  let gone = || {
    Error::not_found(format!(
      "LinkedIn shows no job {id} (it may have been taken down)"
    ))
  };
  let html = api
    .html(&format!("{GUEST}/jobPosting/{id}"), &[])
    .await?
    .ok_or_else(gone)?;
  if html.trim().is_empty() {
    return Err(gone());
  }
  parse::job::job(&html, &id).ok_or_else(|| unexpected("job", &html))
}
