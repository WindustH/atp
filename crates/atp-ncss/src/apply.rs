//! Applying to one job (投递简历) and saving a job (收藏), the way a job's
//! page does it, with the page's form tokens:
//!
//! - applying: for jobs posted on the platform, `GET
//!   /student/applyjobs/issetwebsite` first says whether the employer
//!   takes applications on its own site; then `POST
//!   /student/applyjobs/apply` sends the résumé kept on the site. Jobs of
//!   schools, provinces and partner sites answer with the site the
//!   application continues on (`webSite`);
//! - saving: `POST /student/myjobs/addjobone`.

use atp_core::cli::Change;
use atp_core::text::html_to_text;
use atp_core::{Action, Error, Result, Value, ValueExt};

use crate::api::Api;
use crate::parse::{self, JobPage};
use crate::{jobs, refs};

/// The page of a job, read as the logged-in student.
async fn own_job(api: &Api, id: &str) -> Result<(String, JobPage)> {
  let html = api
    .own_page(&format!("/student/jobs/{id}/detail.html"))
    .await?;
  let page = parse::job(&html, id).ok_or_else(|| jobs::gone(id))?;
  Ok((html, page))
}

/// The token of a form of the page, as `(name, value)`.
fn form_token(html: &str, id: &str) -> Result<(String, String)> {
  parse::token(html, id).ok_or_else(|| {
    Error::upstream("the job's page offers no form for this (the site may have changed)")
  })
}

/// The reason in a refusal, as text.
fn reason(v: &Value) -> String {
  v.first_str(&["reason", "remark", "msg"])
    .map(|r| html_to_text(&r))
    .filter(|r| !r.is_empty())
    .unwrap_or_else(|| "请稍后再试".into())
}

fn summary(page: &JobPage) -> Change {
  let post = &page.post;
  let extra = |key: &str| {
    post
      .extra
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_owned()
  };
  let mut change = Change::new(format!("NCSS job {}:", post.id));
  change
    .row("title", post.title.as_deref().unwrap_or_default())
    .row("company", &extra("company"))
    .row("salary", &extra("salary"))
    .row("city", &extra("city"))
    .row("posted by", &page.source)
    .row("link", post.url.as_deref().unwrap_or_default())
    .row("résumé", "the one you keep on NCSS");
  change
}

/// Apply to one job, after showing it and asking.
pub async fn apply(api: &Api, arg: &str, yes: bool) -> Result<Action> {
  let id = refs::job(arg)?;
  let (html, page) = own_job(api, &id).await?;
  summary(&page).confirm(yes, "Apply to this job")?;
  let referer = format!("/student/jobs/{id}/detail.html");
  if page.own() {
    let params = [("jobId", id.clone())];
    let check = api
      .json("/student/applyjobs/issetwebsite", &params, &referer)
      .await?;
    if check.str("result").as_deref() != Some("true") {
      return Err(Error::input(format!(
        "NCSS does not take the application: {}",
        reason(&check)
      )));
    }
    if let Some(site) = check.str("webSite") {
      return Err(Error::input(format!(
        "the employer takes applications for this job on its own site: {site}"
      )));
    }
  }
  let (name, value) = form_token(&html, "csrf-student-applyjobs-apply")?;
  let form = [("jobId", id.clone()), (name.as_str(), value)];
  let v = api
    .write("/student/applyjobs/apply", &form, &referer)
    .await?;
  if v.str("result").as_deref() != Some("true") {
    return Err(Error::input(format!(
      "NCSS did not take the application: {}",
      reason(&v)
    )));
  }
  let action = Action::done("apply", &id).with_url(refs::job_url(&id));
  Ok(match v.str("webSite") {
    Some(site) => action.with_message(format!(
      "{} takes the application on its own site, where it goes on: {site}",
      page.source
    )),
    None => action.with_message("applied with the résumé you keep on NCSS"),
  })
}

/// Save a job (收藏).
pub async fn favorite(api: &Api, arg: &str) -> Result<Action> {
  let id = refs::job(arg)?;
  let (html, _) = own_job(api, &id).await?;
  let (name, value) = form_token(&html, "csrf-student-myjobs-addjobone")?;
  let form = [("jobId", id.clone()), (name.as_str(), value)];
  let referer = format!("/student/jobs/{id}/detail.html");
  let v = api
    .write("/student/myjobs/addjobone", &form, &referer)
    .await?;
  if v.is_null() || v.as_bool() == Some(false) {
    return Err(Error::upstream("NCSS did not save the job (收藏失败)"));
  }
  let mut action = Action::done("favorite", &id).with_url(refs::job_url(&id));
  if let Some(saved) = v.str("favoriteId") {
    action = action.with_id(saved);
  }
  Ok(action)
}
