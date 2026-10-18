//! Applying to one job (投递), as the job's mobile page does it:
//!
//! - jobs taken elsewhere are reported, not applied to: campus jobs
//!   applied to on the company's campus site (`positionSourceType` 2),
//!   jobs with an application page of their own (`applyType` 4) and jobs
//!   that pass the résumé to the company's own hiring system
//!   (`atsHumanResource`, which the site asks to agree to first);
//! - the résumé is the account's (`getUserDetailHideMobileAndEmail`),
//!   picked with `--resume` when there are several;
//! - `cgate …/bdp/interceptService/intercept` says whether the site stops
//!   the application (its alert is reported);
//! - `POST m.zhaopin.com/api/position/apply` sends it; status 2021 is the
//!   site's face check, which only its app does.

use atp_core::cli::Change;
use atp_core::http::Method;
use atp_core::{Action, Error, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::{jobs, parse, refs, users};

/// The page code of the job page, which the site's requests carry.
const JOB_PAGE: &str = "6020";
/// The status of an application the site wants a face check for.
const FACE_CHECK: i64 = 2021;

/// Why the job is not applied to here, when it is not.
fn elsewhere(detail: &Value) -> Option<String> {
  let flat = detail.at("positionDetail");
  let how = detail.at("state");
  if flat.i64("jobStatus") == Some(4) {
    return Some("the job is no longer open".into());
  }
  if how.i64("positionSourceType") == Some(2) {
    let path = flat.str("deliveryPath").unwrap_or_default();
    return Some(format!(
      "this campus job is applied to on the company's campus site: {path}&channelId=9003"
    ));
  }
  if how.str("applyType").as_deref() == Some("4") {
    let page = flat.str("positionUrl").unwrap_or_default();
    return Some(format!("this job is applied to on its own page: {page}"));
  }
  if flat.bool("atsHumanResource") == Some(true) {
    return Some(
      "this job passes the résumé to the company's own hiring system, which the site asks you to agree to; apply on the site"
        .into(),
    );
  }
  if how.bool("hasAppliedPosition") == Some(true) {
    return Some("you applied to this job already".into());
  }
  None
}

/// The résumé to apply with: `wanted`, or the only one.
fn resume<'a>(account: &'a Value, wanted: Option<&str>) -> Result<&'a Value> {
  let resumes = account.list("resumes");
  let listed = || {
    resumes
      .iter()
      .map(|r| {
        format!(
          "{} ({})",
          r.str("number").unwrap_or_default(),
          r.str("name").unwrap_or_default()
        )
      })
      .collect::<Vec<_>>()
      .join(", ")
  };
  let picked = match wanted {
    Some(number) => resumes
      .iter()
      .find(|r| r.str("number").as_deref() == Some(number))
      .ok_or_else(|| Error::input(format!("no résumé {number}; yours: {}", listed())))?,
    None => match resumes {
      [] => {
        return Err(Error::input(
          "the account has no résumé; create one on the site",
        ));
      }
      [only] => only,
      _ => {
        return Err(Error::input(format!(
          "the account has several résumés; pick one with --resume: {}",
          listed()
        )));
      }
    },
  };
  if picked.i64("postStatus") == Some(0) {
    return Err(Error::input(
      "the résumé is not complete; the site takes complete ones only",
    ));
  }
  Ok(picked)
}

fn summary(post: &Post, resume: &Value) -> Change {
  let extra = |key: &str| {
    post
      .extra
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_owned()
  };
  let mut change = Change::new(format!("Zhilian job {}:", post.id));
  change
    .row("title", post.title.as_deref().unwrap_or_default())
    .row("company", &extra("company"))
    .row("salary", &extra("salary"))
    .row("city", &extra("city"))
    .row("link", post.url.as_deref().unwrap_or_default())
    .row(
      "résumé",
      &format!(
        "{} ({})",
        resume.str("name").unwrap_or_default(),
        resume.str("number").unwrap_or_default()
      ),
    );
  change
}

/// What the site's check before applying says, when it stops it.
async fn stopped(api: &Api, number: &str) -> Option<String> {
  let body = json!({"jobNumber": number, "feature": 1, "scene": 1, "action": 0});
  let v = api
    .gate(Method::POST, "/bdp/interceptService/intercept", Some(&body))
    .await
    .ok()?;
  if v.i64("code") != Some(210) {
    return None;
  }
  let alert: Value = serde_json::from_str(&v.str("message")?).ok()?;
  let alert = alert.at("alert");
  if alert.is_null() {
    return None;
  }
  Some(
    alert
      .first_str(&["content", "title", "message"])
      .unwrap_or_else(|| alert.to_string()),
  )
}

/// Apply to one job, after showing it and asking.
pub async fn apply(api: &Api, arg: &str, wanted: Option<&str>, yes: bool) -> Result<Action> {
  api.require_login()?;
  let (number, detail) = jobs::page(api, arg).await?;
  let post = parse::detail(&detail)
    .ok_or_else(|| Error::not_found(format!("Zhilian shows no job {number}")))?;
  if let Some(reason) = elsewhere(&detail) {
    return Err(Error::input(format!("Zhilian: {reason}")));
  }
  let account = users::account(api).await?;
  let resume = resume(&account, wanted)?;
  summary(&post, resume).confirm(yes, "Apply to this job")?;
  if let Some(alert) = stopped(api, &number).await {
    return Err(Error::input(format!(
      "Zhilian stops the application: {alert}"
    )));
  }
  let body = json!({
    "deliveryChannelType": 1,
    "resumeNumber": resume.str("number"),
    "resumeVersion": resume.str("version").unwrap_or_else(|| "1".into()),
    "positionNumbers": number,
    "cityIds": detail.str("positionDetail.cityId"),
    "ip": "127.0.0.1",
    "askPageCode": 75021,
  });
  let url = refs::job_url(&number);
  let v = api
    .m_post("/api/position/apply", body, &url, JOB_PAGE)
    .await?;
  match v.i64("statusCode").or_else(|| v.i64("code")) {
    Some(200) => Ok(
      Action::done("apply", &number)
        .with_url(url)
        .with_message("applied with your résumé on the site"),
    ),
    Some(FACE_CHECK) => Err(
      Error::verification(
        "Zhilian wants a face check before this application, which only its app does",
      )
      .with_url(url)
      .unclearable(),
    ),
    _ => Err(api.error(&v)),
  }
}
