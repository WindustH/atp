//! Applying to one job (申请接单), as the button of a job page does it: the
//! job must be open (`status` 3) and not applied to yet
//! (`applicationState` 0), the page must see the account as a developer
//! (`role`), the account's real name must be verified (`realname_re` 2),
//! and jobs the page marks for members (`show_type` 1) take developer
//! members only (`vip_type_id` 2); then `recruit/apply` with the job's
//! number. One job at a time, after showing the job and asking. The
//! site's pages show this button only inside the Proginn app.

use atp_core::cli::Change;
use atp_core::{Action, Error, ErrorCode, Post, Result, Value, ValueExt};

use crate::api::Api;
use crate::jobs;
use crate::parse::{self, REAL_NAME_VERIFIED};

/// `status` of a job that takes applications (开放中).
const OPEN: i64 = 3;
/// `vip_type_id` of a developer member (开发者会员).
const DEVELOPER_MEMBER: i64 = 2;

fn denied(text: impl Into<String>) -> Error {
  Error::new(ErrorCode::PermissionDenied, text)
}

/// What the job's page allows, as it checks before its button acts.
fn check_job(v: &Value) -> Result<()> {
  if v.i64("status") != Some(OPEN) {
    let state = v.str("statusName").unwrap_or_else(|| "closed".into());
    return Err(Error::input(format!(
      "the job takes no applications ({state})"
    )));
  }
  if v.i64("applicationState").unwrap_or(0) != 0 {
    return Err(Error::input(
      "you have applied to this job already (已申请接单)",
    ));
  }
  match v.str("role") {
    Some(role) if role != "developer" => Err(denied(format!(
      "Proginn sees this account as `{role}` on this job, not as a developer who may apply"
    ))),
    _ => Ok(()),
  }
}

/// What the account must be, as the page checks it.
fn check_account(job: &Value, account: &Value) -> Result<()> {
  if account.str("uid").is_none() {
    return Err(Error::auth(
      "Proginn knows no account for this session (it may have ended)",
    ));
  }
  if account.i64("realname_re") != Some(REAL_NAME_VERIFIED) {
    return Err(denied(
      "Proginn takes applications only from accounts whose real name it verified (实名认证); \
       verify yours on proginn.com first",
    ));
  }
  if job.i64("show_type") == Some(1) && account.i64("vip_type_id") != Some(DEVELOPER_MEMBER) {
    return Err(denied(
      "this job takes applications from developer members (开发者会员) only",
    ));
  }
  Ok(())
}

fn summary(post: &Post, v: &Value) -> Change {
  let extra = |key: &str| {
    post
      .extra
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_owned()
  };
  let place = match post.extra.get("remote").and_then(Value::as_bool) {
    Some(true) => "remote".to_owned(),
    _ => extra("city"),
  };
  let mut change = Change::new(format!("Proginn job {}:", post.id));
  change
    .row("title", post.title.as_deref().unwrap_or_default())
    .row("company", &extra("company"))
    .row("salary", &extra("salary"))
    .row("where", &place)
    .row("link", post.url.as_deref().unwrap_or_default());
  if v.i64("directionNotMatch") == Some(1) {
    change.row(
      "note",
      "the job's direction is not the one of your profile (职位方向不符)",
    );
  }
  change
}

/// Apply to one job.
pub async fn apply(api: &Api, arg: &str, yes: bool) -> Result<Action> {
  api.require_login()?;
  let v = jobs::recruit(api, arg).await?;
  let post = parse::job(&v).ok_or_else(|| Error::upstream("Proginn's job answer had no id"))?;
  check_job(&v)?;
  let account = api.own("user/getInfo").await?;
  check_account(&v, &account)?;
  summary(&post, &v).confirm(yes, "Apply to this job")?;
  let id = v.str("id").unwrap_or_else(|| post.id.clone());
  api.write("recruit/apply", &[("recruitId", id)]).await?;
  let mut action = Action::done("apply", &post.id)
    .with_message("applied (申请接单); the client picks whom to hire among those who apply");
  if let Some(url) = post.url {
    action = action.with_url(url);
  }
  Ok(action)
}
