//! Meituan's jobs (search rows and job pages alike).

use atp_core::text::from_millis;
use atp_core::{Value, ValueExt};

use super::{Job, joined, names, text};
use crate::jobs::Kind;
use crate::meituan::WWW;

const COMPANY: &str = "美团";

/// The kind of a `jobType`: 1 应届, 2 实习, 3 社招.
fn recruit(code: Option<&str>) -> Option<Kind> {
  match code? {
    "1" => Some(Kind::Campus),
    "2" => Some(Kind::Intern),
    "3" => Some(Kind::Social),
    _ => None,
  }
}

pub fn job_url(id: &str, kind: Option<Kind>) -> String {
  let highlight = if kind == Some(Kind::Social) {
    "social"
  } else {
    "campus"
  };
  format!("{WWW}/web/position/detail?jobUnionId={id}&highlightType={highlight}")
}

/// A job, from the search or its page.
pub fn job(v: &Value) -> Option<Job> {
  let id = v.str("jobUnionId")?;
  let kind = recruit(v.str("jobType").as_deref());
  let category = [v.str("jobFamily"), v.str("jobFamilyGroup")]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("/");
  let departments = names(v, "department", Some("name"));
  Some(Job {
    url: job_url(&id, kind),
    title: v.str("name"),
    text: joined(&[
      text(v, "desc"),
      text(v, "jobDuty"),
      text(v, "jobRequirement"),
      text(v, "precedence"),
      text(v, "highLight"),
      text(v, "otherInfo"),
    ]),
    company: COMPANY.into(),
    site: WWW.into(),
    cities: names(v, "cityList", Some("name")),
    recruit: kind,
    category: (!category.is_empty()).then_some(category),
    department: (!departments.is_empty()).then(|| departments.join("、")),
    batch: v.str("projectName"),
    experience: v.str("workYear"),
    created_at: v.i64("firstPostTime").and_then(from_millis),
    updated_at: v.i64("refreshTime").and_then(from_millis),
    raw: v.clone(),
    id,
    ..Job::default()
  })
}
