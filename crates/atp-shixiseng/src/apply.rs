//! Applying to one job (投递简历), as the job page's dialog does it:
//!
//! - jobs taken elsewhere are reported, not applied to: jobs already
//!   applied to, closed jobs, jobs with an application page of their own
//!   (`apply_link`, or the options' `url`);
//! - `GET deliver/v3.0/deliver/options?inuuid=` names the account's
//!   résumés (`group_uuid`, `stype`, whether the job takes them) and what
//!   the dialog fills in: when you can start (`report_time`), how long you
//!   can stay (`internship_time`) and days a week (`days`);
//! - `POST deliver/v3.0/deliver` sends it; jobs the site gathered from
//!   other sites (`xiaozhao_type` 2, 3 and 4) go to `deliver/xz`.

use atp_core::cli::Change;
use atp_core::{Action, Error, Post, Result, Value, ValueExt, json};

use crate::api::{Api, Route};
use crate::filters::{Start, Stay};
use crate::{jobs, parse, refs};

const OPTIONS: &str = "/api/deliver/v3.0/deliver/options";
const DELIVER: &str = "/api/deliver/v3.0/deliver";
const DELIVER_OUTSIDE: &str = "/api/deliver/v3.0/deliver/xz";
/// `xiaozhao_type` of jobs gathered from other sites.
const OUTSIDE: [i64; 3] = [2, 3, 4];
/// What the site sends as where the application came from.
const POSITION: &str = "pc_default";

/// What the dialog asks, as given on the command line.
#[derive(Debug, Clone, Default)]
pub struct Answers {
  /// A résumé's `group_uuid`.
  pub resume: Option<String>,
  pub days: Option<u8>,
  pub start: Option<Start>,
  pub stay: Option<Stay>,
}

/// Why the job is not applied to here, when it is not.
fn elsewhere(info: &Value) -> Option<String> {
  if info.str("delivered").as_deref() == Some("1") {
    return Some("you applied to this job already".into());
  }
  if info.str("overdue").as_deref() == Some("1") {
    return Some("the job is no longer open".into());
  }
  let link = info.str("apply_link").filter(|l| !l.trim().is_empty())?;
  Some(format!("this job is applied to on its own page: {link}"))
}

/// A résumé as the options name it.
fn describe(resume: &Value) -> String {
  let name = resume
    .first_str(&["name", "title", "resume_name"])
    .unwrap_or_else(|| "résumé".into());
  format!("{} ({name})", resume.str("group_uuid").unwrap_or_default())
}

/// The résumé to apply with: `wanted`, or the only one the job takes.
fn resume<'a>(options: &'a Value, wanted: Option<&str>) -> Result<&'a Value> {
  let resumes = options.list("resume");
  let listed = || resumes.iter().map(describe).collect::<Vec<_>>().join(", ");
  if let Some(wanted) = wanted {
    let picked = resumes
      .iter()
      .find(|r| r.str("group_uuid").as_deref() == Some(wanted))
      .ok_or_else(|| Error::input(format!("no résumé {wanted}; yours: {}", listed())))?;
    if picked.bool("deliver_able") == Some(false) {
      return Err(Error::input(
        "this résumé does not meet the job's requirements (该简历不符合职位投递要求)",
      ));
    }
    return Ok(picked);
  }
  let usable: Vec<&Value> = resumes
    .iter()
    .filter(|r| r.bool("deliver_able") != Some(false))
    .collect();
  match usable.as_slice() {
    [] if resumes.is_empty() => Err(Error::input(
      "the account has no résumé; create one on the site",
    )),
    [] => Err(Error::input(
      "none of your résumés meets the job's requirements (该简历不符合职位投递要求)",
    )),
    [only] => Ok(only),
    _ => Err(Error::input(format!(
      "the account has several résumés; pick one with --resume: {}",
      listed()
    ))),
  }
}

/// What goes with the résumé: days a week, when you start, how long you
/// stay; given, else as the dialog fills them in.
struct Terms {
  days: u64,
  start: String,
  stay: String,
}

fn terms(options: &Value, answers: &Answers) -> Terms {
  Terms {
    days: answers
      .days
      .map(u64::from)
      .or_else(|| options.u64("days").filter(|d| *d > 0))
      .unwrap_or(1),
    start: answers
      .start
      .map(|s| s.label().to_owned())
      .or_else(|| options.str("report_time"))
      .unwrap_or_else(|| "一周内".into()),
    stay: answers
      .stay
      .map(|s| s.label().to_owned())
      .or_else(|| options.str("internship_time"))
      .unwrap_or_else(|| "3个月以内".into()),
  }
}

fn summary(post: &Post, resume: &Value, terms: &Terms) -> Change {
  let extra = |key: &str| {
    post
      .extra
      .get(key)
      .and_then(|v| v.as_str())
      .unwrap_or_default()
      .to_owned()
  };
  let mut change = Change::new(format!("Shixiseng job {}:", post.id));
  change
    .row("title", post.title.as_deref().unwrap_or_default())
    .row("company", &extra("company"))
    .row("salary", &extra("salary"))
    .row("city", &extra("city"))
    .row("link", post.url.as_deref().unwrap_or_default())
    .row("résumé", &describe(resume))
    .row("days/week", &terms.days.to_string())
    .row("start", &terms.start)
    .row("stay", &terms.stay);
  change
}

/// The body of the application, as the dialog sends it.
fn body(id: &str, info: &Value, resume: &Value, terms: &Terms) -> (&'static str, Value) {
  let group = resume.str("group_uuid");
  let code = info.i64("xiaozhao_type").unwrap_or(0);
  if OUTSIDE.contains(&code) {
    let body = json!({
      "days": terms.days,
      "internship_time": terms.stay,
      "uuid": id,
      "report_time": terms.start,
      "group_uuid": group,
      "position": POSITION,
    });
    return (DELIVER_OUTSIDE, body);
  }
  let body = json!({
    "days": terms.days.to_string(),
    "group_uuid": group,
    "internship_time": terms.stay,
    "inuuid": id,
    "report_time": terms.start,
    "stype": resume.at("stype"),
    "position": POSITION,
    "act": "",
    "deliver_type": null,
  });
  (DELIVER, body)
}

/// Apply to one job, after showing it and asking.
pub async fn apply(api: &Api, arg: &str, answers: &Answers, yes: bool) -> Result<Action> {
  api.require_login()?;
  let id = refs::job(arg)?;
  let url = refs::job_url(&id);
  let info = jobs::info(api, &id).await?;
  let post = parse::detail(&info, &id)
    .ok_or_else(|| Error::not_found(format!("Shixiseng shows no job at {url}")))?;
  if let Some(reason) = elsewhere(&info) {
    return Err(Error::input(format!("Shixiseng: {reason}")));
  }
  let options = api
    .get(Route::Deliver, OPTIONS, &[("inuuid", id.clone())], &url)
    .await?;
  let options = options.at("msg");
  if let Some(page) = options.str("url")
    && options.bool("can_deliver") == Some(true)
  {
    return Err(Error::input(format!(
      "Shixiseng: this job is applied to on the company's page: {page}"
    )));
  }
  let resume = resume(options, answers.resume.as_deref())?;
  let terms = terms(options, answers);
  summary(&post, resume, &terms).confirm(yes, "Apply to this job")?;
  let (path, body) = body(&id, &info, resume, &terms);
  api.post(Route::Deliver, path, &body, &url).await?;
  Ok(
    Action::done("apply", &id)
      .with_url(url)
      .with_message("applied with your résumé on the site"),
  )
}
