//! Companies, which are LinkedIn's users here: a company's public page
//! (`/company/<name>`), and the numbers its jobs are searched by, read
//! from that page once and kept a month (they do not change).

use std::time::Duration;

use atp_core::{Error, Result, User};

use crate::api::{Api, unexpected};
use crate::parse;
use crate::refs::{self, CompanyRef};

const CACHE_TTL: Duration = Duration::from_secs(30 * 24 * 3600);

/// The cache key of a company's numbers, by the name in its link.
fn cache_key(name: &str) -> String {
  let name: String = name
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
    .collect();
  format!("linkedin-company-{name}")
}

/// A company's page.
async fn page(api: &Api, company: &CompanyRef) -> Result<String> {
  let url = company.url();
  api
    .html(&url, &[])
    .await?
    .ok_or_else(|| Error::not_found(format!("LinkedIn has no company page at {url}")))
}

/// A company's page, as a user.
pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let company = refs::company(arg)?;
  let html = page(api, &company).await?;
  let (CompanyRef::Id(asked) | CompanyRef::Name(asked)) = &company;
  let user = parse::company::company(&html, asked).ok_or_else(|| unexpected("company", &html))?;
  if let Some(ids) = parse::company::job_ids(&html) {
    api.ctx.store.cache_put(&cache_key(&user.id), &ids);
  }
  Ok(user)
}

/// The company numbers a company's jobs are searched by (`f_C`): a number
/// as given, else the ones its page names.
pub async fn job_ids(api: &Api, arg: &str) -> Result<String> {
  let name = match refs::company(arg)? {
    CompanyRef::Id(id) => return Ok(id),
    CompanyRef::Name(name) => name,
  };
  let key = cache_key(&name);
  if let Some(ids) = api.ctx.store.cache_get::<String>(&key, CACHE_TTL) {
    return Ok(ids);
  }
  let html = page(api, &CompanyRef::Name(name)).await?;
  let ids = parse::company::job_ids(&html).ok_or_else(|| unexpected("company number", &html))?;
  api.ctx.store.cache_put(&key, &ids);
  Ok(ids)
}
