//! Your own lists, as the job seeker's 互动 page shows them: jobs you
//! marked 感兴趣 (`interaction/geekGetJob`, tag 4: saved jobs) and jobs you
//! chatted about (tag 5, 沟通过), your applications
//! (`resume/geekDeliverList`, 已投递) and interviews you were invited to
//! (`interview/list`, 面试); and marking a job 感兴趣 or taking the mark
//! off (`geekTag/job/interest`, `flag` 1 or 0, as the job page and the
//! list's 取消感兴趣 button send it, with the job's `securityId` and
//! `lid`).

use atp_core::{Action, Error, Page, PageReq, Post, Result, Value, ValueExt};

use crate::api::{Api, Call, GEEK};
use crate::{jobs, parse, refs};

/// The tags of `geekGetJob`.
const SAVED: &str = "4";
const CHATTED: &str = "5";

/// The next page, by the list's `hasMore` or its total.
fn next(v: &Value, page: u64, seen: usize, total_key: &str) -> Option<String> {
  let more = match (v.bool("hasMore"), v.u64(total_key)) {
    (Some(more), _) => more,
    (None, Some(total)) => (seen as u64) < total,
    (None, None) => false,
  };
  (more && seen > 0).then(|| (page + 1).to_string())
}

/// Cards of a list: jobs, as posts of `kind`.
fn cards(list: &[Value], kind: &str) -> Vec<Post> {
  list
    .iter()
    .filter_map(parse::card)
    .map(|mut post| {
      post.kind = kind.into();
      post
    })
    .collect()
}

async fn interactions(api: &Api, tag: &str, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let page = req.number_or(1);
  let query = [
    ("page", page.to_string()),
    ("tag", tag.to_owned()),
    ("isActive", "true".to_owned()),
  ];
  let v = api
    .get("/wapi/zprelation/interaction/geekGetJob", &query, GEEK)
    .await?;
  let items = cards(v.list("cardList"), "job");
  let cursor = next(&v, page, items.len(), "totalCount");
  Ok(Page::new(items, cursor))
}

/// Jobs you marked 感兴趣, the site's saved jobs.
pub async fn favorites(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  interactions(api, SAVED, req).await
}

/// Jobs whose recruiters you chatted with (沟通过).
pub async fn chatted(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  interactions(api, CHATTED, req).await
}

/// Your applications (已投递): the jobs you sent a résumé for.
pub async fn applied(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let page = req.number_or(1);
  let query = [("page", page.to_string())];
  let v = api
    .get("/wapi/zprelation/resume/geekDeliverList", &query, GEEK)
    .await?;
  let items = cards(v.list("cardList"), "application");
  let cursor = next(&v, page, items.len(), "totalCount");
  Ok(Page::new(items, cursor))
}

/// Interviews recruiters invited you to (面试).
pub async fn interviews(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  api.require_login()?;
  let page = req.number_or(1);
  let query = [("page", page.to_string())];
  let v = api
    .get("/wapi/zpinterview/geek/interview/list", &query, GEEK)
    .await?;
  let items: Vec<Post> = v
    .list("interviewList")
    .iter()
    .map(parse::interview)
    .collect();
  let cursor = next(&v, page, items.len(), "totalCount");
  Ok(Page::new(items, cursor))
}

/// Mark a job 感兴趣 (save it) or take the mark off. The ids the site
/// wants come with the links lists print; for other links, from the
/// job's page.
pub async fn favorite(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  api.require_login()?;
  let mut job = refs::job(arg)?;
  if job.security_id.is_none() {
    job = jobs::job(api, arg).await?.job;
  }
  let security = job
    .security_id
    .clone()
    .ok_or_else(|| Error::upstream("the job's page named no securityId to save the job with"))?;
  let query = [
    ("securityId", security),
    ("jobId", job.id.clone()),
    ("expectId", String::new()),
    ("tag", SAVED.to_owned()),
    ("lid", job.lid.clone().unwrap_or_default()),
  ];
  let form = [("flag", if undo { "0" } else { "1" }.to_owned())];
  let url = job.url();
  let call = Call {
    path: "/wapi/zprelation/geekTag/job/interest",
    query: &query,
    form: Some(&form),
    from: &url,
  };
  api.write(&call).await?;
  let action = if undo { "unfavorite" } else { "favorite" };
  Ok(Action::done(action, &job.id).with_url(url.clone()))
}
