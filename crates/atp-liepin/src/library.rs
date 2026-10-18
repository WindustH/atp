//! Your own lists, as `c.liepin.com/record` shows them: saved jobs
//! (`c.job-favorite.get-favorite-job-page`, 收藏), your applications
//! (`capply.pc.apply-list`, 我的投递) and the invitations to apply
//! recruiters sent (`capply.platform.invitation.list`, 邀请应聘); and
//! saving a job or taking it out (`c.job-favorite.favorite` / `cancel`).

use atp_core::{Action, Page, PageReq, Post, Result, ValueExt};

use crate::api::Api;
use crate::{jobs, parse};

/// Rows per page of the record pages.
const PAGE: u64 = 20;

/// The next page number when `total` rows are more than the pages so far.
fn next(page: u64, size: u64, total: Option<u64>, items: usize) -> Option<String> {
  let more = match total {
    Some(total) => (page + 1) * size < total,
    None => items as u64 >= size,
  };
  (items > 0 && more).then(|| (page + 1).to_string())
}

pub async fn favorites(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(0);
  let form = [
    ("curPage", page.to_string()),
    ("pageSize", PAGE.to_string()),
  ];
  let v = api
    .own("c.job-favorite.get-favorite-job-page", &form)
    .await?;
  let items: Vec<Post> = v
    .list("datas")
    .iter()
    .map(|row| row.at("jobCardDto"))
    .filter(|card| !card.at("job").is_null())
    .map(parse::card)
    .collect();
  let cursor = next(page, PAGE, v.u64("totalCnt"), items.len());
  Ok(Page::new(items, cursor))
}

/// Save a job (收藏) or take it out; the job's page names its ids.
pub async fn favorite(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  api.require_login()?;
  let job = jobs::job(api, arg).await?;
  let name = if undo {
    "c.job-favorite.cancel"
  } else {
    "c.job-favorite.favorite"
  };
  let form = [("jobId", job.id.clone()), ("jobKind", job.kind.clone())];
  api.write(name, &form, &job.url).await?;
  let action = if undo { "unfavorite" } else { "favorite" };
  Ok(Action::done(action, &job.post.id).with_url(job.url))
}

/// `applied --status`: the site's tabs.
pub fn status_code(status: &str) -> &'static str {
  match status {
    "applied" => "0",
    "viewed" => "1",
    "rejected" => "2",
    "interview" => "3",
    _ => "",
  }
}

pub async fn applied(api: &Api, status: Option<&str>, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(0);
  let form = [
    ("curPage", page.to_string()),
    (
      "applyStatus",
      status.map(status_code).unwrap_or_default().to_owned(),
    ),
  ];
  let v = api.own("capply.pc.apply-list", &form).await?;
  let items: Vec<Post> = v.list("list").iter().map(parse::application).collect();
  let size = v.u64("pageSize").filter(|n| *n > 0).unwrap_or(PAGE);
  let cursor = next(page, size, v.u64("totalCount"), items.len());
  Ok(Page::new(items, cursor))
}

/// The invitations, newest first; the cursor is the site's `lastId`.
pub async fn invitations(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let last = req.cursor.clone().unwrap_or_else(|| "0".into());
  let v = api
    .own("capply.platform.invitation.list", &[("lastId", last)])
    .await?;
  let items: Vec<Post> = v.list("datas").iter().map(parse::invitation).collect();
  let cursor = (v.bool("hasNextPage") == Some(true))
    .then(|| v.str("lastId"))
    .flatten();
  Ok(Page::new(items, cursor))
}
