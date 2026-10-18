//! Companies, which are BOSS Zhipin's users here: a company's page
//! (`/gongsi/<id>.html`) and the page of its jobs (`/gongsi/job/<id>.html`),
//! read in the browser tab ([`crate::pages`]); with a login, the
//! companies you follow (公司收藏: `brand/focus/querylist`) and following
//! or leaving one (`brand/focus.json`, as the 收藏 button sends it); and
//! your own account (`zpuser/wap/getUserInfo.json`).

use atp_core::{Action, Error, Page, PageReq, Post, Result, User, ValueExt};

use crate::api::{Api, Call, GEEK};
use crate::{pages, parse, refs};

/// Companies per page of your followed list, as the page asks for them.
const PAGE: u64 = 15;

async fn page(api: &Api, url: &str, id: &str) -> Result<(User, Vec<Post>)> {
  let page = pages::company(api, url).await?;
  parse::company_page(&page, id).ok_or_else(|| {
    Error::upstream(format!(
      "BOSS Zhipin's company page {url} came without the company ATP looks for (the page may have changed)"
    ))
  })
}

/// A company's page: name, logo, funding stage, size, industry,
/// introduction.
pub async fn company(api: &Api, arg: &str) -> Result<User> {
  let id = refs::company(arg)?;
  Ok(page(api, &refs::company_url(&id), &id).await?.0)
}

/// The jobs a company lists, from the page of its jobs.
pub async fn jobs(api: &Api, arg: &str) -> Result<Page<Post>> {
  let id = refs::company(arg)?;
  Ok(Page::last(
    page(api, &refs::company_jobs_url(&id), &id).await?.1,
  ))
}

/// The account the session belongs to.
pub async fn whoami(api: &Api) -> Result<User> {
  api.require_login()?;
  let v = api
    .get("/wapi/zpuser/wap/getUserInfo.json", &[], GEEK)
    .await?;
  let id = v
    .str("encryptUserId")
    .or_else(|| v.str("userId"))
    .ok_or_else(|| Error::upstream("BOSS Zhipin named no account for this session"))?;
  if v.i64("identity") == Some(1) {
    return Err(
      Error::new(
        atp_core::ErrorCode::PermissionDenied,
        "this BOSS Zhipin account is in its recruiter role; ATP works with the job seeker's side",
      )
      .with_hint("switch the account to the job seeker role (牛人身份) in the BOSS app"),
    );
  }
  *api.me.borrow_mut() = Some(id.clone());
  Ok(User {
    id,
    name: v
      .first_str(&["name", "showName", "nickName"])
      .unwrap_or_default(),
    avatar: v.first_str(&["largeAvatar", "avatar", "tinyAvatar"]),
    ..User::default()
  })
}

/// The companies you follow (公司收藏), newest first. `user` is you: the
/// CLI asks `whoami` when no user is given; the site shows no one else's.
pub async fn following(api: &Api, user: &str, req: &PageReq) -> Result<Page<User>> {
  api.require_login()?;
  if api.me.borrow().as_deref() != Some(user) {
    return Err(Error::input(
      "BOSS Zhipin shows only the companies you follow yourself: run `following` without a user",
    ));
  }
  let page = req.number_or(1);
  let query = [("page", page.to_string()), ("pageSize", PAGE.to_string())];
  let v = api
    .get("/wapi/zpgeek/brand/focus/querylist", &query, GEEK)
    .await?;
  let items: Vec<User> = v
    .list("brandList")
    .iter()
    .filter_map(parse::followed)
    .collect();
  let more = !items.is_empty()
    && match v.u64("brandCount") {
      Some(total) => page * PAGE < total,
      None => items.len() as u64 >= PAGE,
    };
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// Follow a company (收藏) or leave it: `flag` 0 follows and 1 leaves,
/// as the button of the followed list sends it.
pub async fn follow(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::company(arg)?;
  let form = [
    ("brandId", id.clone()),
    ("flag", if undo { "1" } else { "0" }.to_owned()),
  ];
  let call = Call {
    path: "/wapi/zpgeek/brand/focus.json",
    query: &[],
    form: Some(&form),
    from: GEEK,
  };
  let v = api.write(&call).await?;
  if v.bool("result") == Some(false) {
    return Err(Error::upstream(
      "BOSS Zhipin did not take the change (the site's page reloads in this case)",
    ));
  }
  let action = if undo { "unfollow" } else { "follow" };
  Ok(Action::done(action, &id).with_url(refs::company_url(&id)))
}
