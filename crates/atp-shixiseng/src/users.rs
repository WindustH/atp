//! The logged-in account (`account/v3.0/baseinfo`) and companies, which
//! are Shixiseng's users here: their pages (`company/info/wxz`) and the
//! company search of the search page (`/company/search`, 20 a page).

use atp_core::{Error, Page, PageReq, Query, Result, User, ValueExt};

use crate::api::{Api, Route, WWW};
use crate::jobs::NATIONWIDE;
use crate::{parse, refs};

const BASEINFO: &str = "/api/account/v3.0/baseinfo";
const COMPANY: &str = "/api/interns/v3.0/company/info/wxz";
const SEARCH: &str = "/company/search";

pub async fn whoami(api: &Api) -> Result<User> {
  api.require_login()?;
  let v = api
    .get(Route::Account, BASEINFO, &[], &format!("{WWW}/"))
    .await?;
  parse::account(v.at("msg")).ok_or_else(|| {
    Error::auth("Shixiseng did not name the logged-in account").with_hint(api.ctx.login_hint())
  })
}

/// A company's page; read once a run.
pub async fn company(api: &Api, arg: &str) -> Result<User> {
  let id = refs::company(arg)?;
  let known = api
    .companies
    .lock()
    .ok()
    .and_then(|known| known.get(&id).cloned());
  if let Some(user) = known {
    return Ok(user);
  }
  let url = refs::company_url(&id);
  let v = api
    .get(Route::Interns, COMPANY, &[("uuid", id.clone())], &url)
    .await?;
  let user = parse::company(v.at("msg"), &id)
    .ok_or_else(|| Error::not_found(format!("Shixiseng shows no company at {url}")))?;
  if let Ok(mut known) = api.companies.lock() {
    known.insert(id, user.clone());
  }
  Ok(user)
}

/// Companies whose name matches, all over China.
pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<User>> {
  let page = req.number_or(1);
  let params = [
    ("p", page.to_string()),
    ("kw", query.keyword.clone()),
    ("city", NATIONWIDE.to_owned()),
  ];
  let referer = format!("{WWW}/interns?type=company");
  let v = api.get(Route::Site, SEARCH, &params, &referer).await?;
  let msg = v.at("msg");
  let rows = msg.list("data");
  let items: Vec<User> = rows.iter().filter_map(parse::company_card).collect();
  let size = msg.u64("pageNumber").filter(|n| *n > 0).unwrap_or(20);
  let total = msg.u64("total").unwrap_or(0);
  let more = !rows.is_empty() && page * size < total;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}
