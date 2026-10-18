//! Companies, which are 51job's users here: a company's page on
//! jobs.51job.com (`/all/co<code>.html`) with its introduction and the
//! jobs it lists ([`crate::pages`]).

use atp_core::{Error, Page, Post, Result, User};

use crate::api::Api;
use crate::{pages, parse, refs};

async fn page(api: &Api, arg: &str) -> Result<(User, Vec<Post>)> {
  let code = refs::company(arg)?;
  let url = refs::company_url(&code);
  let page = pages::company(api, &url).await?;
  parse::company_page(&page, &code).ok_or_else(|| {
    Error::upstream(format!(
      "51job's company page {url} came without the company ATP looks for (the page may have changed)"
    ))
  })
}

/// A company's page: name, logo, ownership, size, industry, introduction.
pub async fn company(api: &Api, arg: &str) -> Result<User> {
  Ok(page(api, arg).await?.0)
}

/// The jobs a company's page lists, all on one page.
pub async fn jobs(api: &Api, arg: &str) -> Result<Page<Post>> {
  Ok(Page::last(page(api, arg).await?.1))
}
