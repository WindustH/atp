//! Applying to one job (投递), the way the job page's button does it:
//! `cbff.apply.before-apply` names anything that must happen first (a
//! résumé to complete, a résumé the site turned down, a company you
//! block, consent for recruiters abroad); `capply.apply.apply-data`
//! names your default online résumé; `capply.platform.apply-for-pc`
//! sends it. One job at a time, after showing the job and asking.

use atp_core::cli::Change;
use atp_core::{Action, Error, Result, ValueExt};

use crate::api::Api;
use crate::jobs::{self, Job};

/// What `before-apply` asks for first, as the site words it.
fn guide(key: &str) -> Option<&'static str> {
  Some(match key {
    "apply_guide_complete" => "your online résumé is not complete; finish it on liepin.com first",
    "apply_guide_res_audit" => {
      "Liepin's review turned your résumé down; change it on liepin.com first"
    }
    "apply_guide_open_comp_shield" => {
      "you block this company (屏蔽公司); unblock it on liepin.com to apply"
    }
    "apply_guide_data_export_auth" => {
      "this recruiter is abroad and the site needs your consent to show them your résumé; \
       give it on the job's page"
    }
    _ => return None,
  })
}

fn summary(job: &Job) -> Change {
  let post = &job.post;
  let extra = |key: &str| {
    post
      .extra
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_owned()
  };
  let mut change = Change::new(format!("Liepin job {}:", post.id));
  change
    .row("title", post.title.as_deref().unwrap_or_default())
    .row("company", &extra("company"))
    .row("salary", &extra("salary"))
    .row("city", &extra("city"))
    .row("link", &job.url)
    .row("résumé", "your default online résumé, no attachment");
  change
}

/// Apply to one job with the default online résumé.
pub async fn apply(api: &Api, arg: &str, yes: bool) -> Result<Action> {
  api.require_login()?;
  let job = jobs::job(api, arg).await?;
  summary(&job).confirm(yes, "Apply to this job")?;
  let form = [("jobId", job.id.clone()), ("jobKind", job.kind.clone())];
  let before = api.form("cbff.apply.before-apply", &form).await?;
  if let Some(need) = before.str("bizKey").as_deref().and_then(guide) {
    return Err(Error::input(format!(
      "Liepin cannot take the application: {need}"
    )));
  }
  let data = api.form("capply.apply.apply-data", &form).await?;
  let resume = data
    .str("applyResumeInfo.encryDefaultResumeId")
    .ok_or_else(|| {
      Error::input("Liepin names no online résumé of yours to apply with; create one on liepin.com")
    })?;
  let send = [
    ("jobId", job.id.clone()),
    ("jobKind", job.kind.clone()),
    ("useApplyPrior", "false".to_owned()),
    ("resIdEncode", resume),
    ("encryAttachmentId", String::new()),
  ];
  api
    .write("capply.platform.apply-for-pc", &send, &job.url)
    .await?;
  Ok(
    Action::done("apply", &job.post.id)
      .with_url(job.url)
      .with_message("applied with your default online résumé"),
  )
}
