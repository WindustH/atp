//! What the account keeps: saved jobs (收藏, `account/v3.0/collect/list`,
//! `collect/add`, `collect/cancle` as the site spells it) and your
//! applications (投递记录, `deliver/v3.0/deliver/list`), 10 a page each.

use atp_core::{Action, Page, PageReq, Post, Result, ValueExt, json};

use crate::api::{Api, Route, WWW};
use crate::parse::{self, Recruit};
use crate::{jobs, refs};

const SAVED: &str = "/api/account/v3.0/collect/list";
const SAVE: &str = "/api/account/v3.0/collect/add";
const UNSAVE: &str = "/api/account/v3.0/collect/cancle";
const APPLIED: &str = "/api/deliver/v3.0/deliver/list";
/// Rows per page, as the account's pages ask for them.
const PAGE: u64 = 10;

/// Your saved jobs, internships and campus jobs together.
pub async fn favorites(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let page = req.number_or(1);
  let params = [
    ("page", page.to_string()),
    ("size", PAGE.to_string()),
    ("type", "intern".to_owned()),
    ("version", "v2".to_owned()),
    ("sub", "sx_xz".to_owned()),
  ];
  let referer = format!("{WWW}/my/collect");
  let v = api.get(Route::Account, SAVED, &params, &referer).await?;
  let items: Vec<Post> = v.list("msg").iter().filter_map(parse::saved).collect();
  let pages = v.u64("total_page").unwrap_or(0);
  let more = !items.is_empty() && page < pages;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// Save a job or take it out. Saving names the kind of job, which its page
/// tells.
pub async fn favorite(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  api.require_login()?;
  let id = refs::job(arg)?;
  let url = refs::job_url(&id);
  if undo {
    api
      .post(Route::Account, UNSAVE, &json!({"uuid": id}), &url)
      .await?;
    return Ok(Action::done("unfavorite", &id).with_url(url));
  }
  let info = jobs::info(api, &id).await?;
  let recruit = Recruit::from_code(info.i64("xiaozhao_type").unwrap_or(0));
  let body = json!({"uuid": id, "type": recruit.saved_type()});
  api.post(Route::Account, SAVE, &body, &url).await?;
  Ok(Action::done("favorite", &id).with_url(url))
}

/// Your applications, all of them or those at one stage (`stage`, the
/// site's tab: `delivered` for all).
pub async fn applied(api: &Api, stage: &str, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let page = req.number_or(1);
  let params = [
    ("page", page.to_string()),
    ("stype", stage.to_owned()),
    ("intern_type", String::new()),
  ];
  let referer = format!("{WWW}/my/delivered");
  let v = api.get(Route::Deliver, APPLIED, &params, &referer).await?;
  let items: Vec<Post> = v
    .list("msg")
    .iter()
    .filter_map(parse::application)
    .collect();
  let total = v.u64("total").unwrap_or(0);
  let more = !items.is_empty() && page * PAGE < total;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}
