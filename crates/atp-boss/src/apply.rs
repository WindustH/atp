//! Applying to one job, which on BOSS Zhipin is greeting its recruiter
//! (打招呼): `zpgeek/friend/add.json`, as the job page's 立即沟通 button
//! sends it, opens a chat with the recruiter, and the site sends your
//! account's default greeting (set in the BOSS app); ATP adds no text of
//! its own. One job at a time, after showing the job and asking.

use atp_core::cli::Change;
use atp_core::{Action, Error, Result};

use crate::api::{Api, Call};
use crate::jobs::{self, Job};

fn summary(found: &Job) -> Change {
  let post = &found.post;
  let extra = |key: &str| {
    post
      .extra
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_owned()
  };
  let mut change = Change::new(format!("BOSS Zhipin job {}:", post.id));
  change
    .row("title", post.title.as_deref().unwrap_or_default())
    .row("company", &extra("company"))
    .row("salary", &extra("salary"))
    .row("city", &extra("city"))
    .row("link", &found.job.url())
    .row(
      "greeting",
      "your account's default greeting (打招呼), which starts a chat with the recruiter",
    );
  change
}

/// Greet the recruiter of one job, the way the job's page does.
pub async fn apply(api: &Api, arg: &str, yes: bool) -> Result<Action> {
  api.require_login()?;
  let found = jobs::job(api, arg).await?;
  summary(&found).confirm(yes, "Greet the recruiter of this job")?;
  let job = &found.job;
  let security = job
    .security_id
    .clone()
    .ok_or_else(|| Error::upstream("the job's page named no securityId to start a chat with"))?;
  let query = [
    ("securityId", security),
    ("jobId", job.id.clone()),
    ("lid", job.lid.clone().unwrap_or_default()),
  ];
  let url = job.url();
  let call = Call {
    path: "/wapi/zpgeek/friend/add.json",
    query: &query,
    form: Some(&[]),
    from: &url,
  };
  api.write(&call).await?;
  Ok(
    Action::done("apply", &job.id)
      .with_url(url.clone())
      .with_message("greeted the recruiter; the chat is in `chats`"),
  )
}
