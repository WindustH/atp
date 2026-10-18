//! Beisen's answers: the site its page describes, and jobs (search rows
//! and job pages alike).

use atp_core::jobs::Salary;
use atp_core::{Value, ValueExt};

use super::{Job, job_type, joined, local, names, text};
use crate::beisen::Portal;
use crate::jobs::Kind;
use crate::sites::Site;

/// The site of a page: `var BSGlobal = {…}`, its portal id and company.
pub fn portal(html: &str) -> Option<Portal> {
  let start = html.find("var BSGlobal")?;
  let body = &html[start..];
  let body = &body[body.find('{')?..];
  let v: Value = serde_json::Deserializer::from_str(body)
    .into_iter::<Value>()
    .next()?
    .ok()?;
  Some(Portal {
    id: v.str("PortalId")?,
    company: v
      .first_str(&["tenantInfo.Alias", "tenantInfo.Name", "Name"])
      .unwrap_or_default(),
  })
}

/// The kind of a `CategoryId`: 1 社招, 2 校招, 3 实习.
fn recruit(id: Option<&str>) -> Option<Kind> {
  match id? {
    "1" => Some(Kind::Social),
    "2" => Some(Kind::Campus),
    "3" => Some(Kind::Intern),
    _ => None,
  }
}

pub fn job_url(site: &Site, id: &str, kind: Option<Kind>) -> String {
  let path = kind.map_or("campus", Kind::name);
  format!("{}/{path}/detail?jobAdId={id}", site.origin)
}

/// A day of a local time, unless it stands for none: the empty
/// `0001-01-01`, or a year far ahead (`2222-02-02`, open until filled).
fn date(v: &Value, path: &str) -> Option<String> {
  let text = v.str(path)?;
  let day = text.get(..10)?;
  let year: u32 = day.get(..4)?.parse().ok()?;
  (1900..2100).contains(&year).then(|| day.to_owned())
}

/// A job, from the search or its page.
pub fn job(v: &Value, site: &Site, portal: &Portal) -> Option<Job> {
  let id = v.str("Id")?;
  let kind = recruit(v.str("CategoryId").as_deref());
  let company = if portal.company.is_empty() {
    site.host().to_owned()
  } else {
    portal.company.clone()
  };
  let classes: Vec<String> = ["ClassificationOne", "ClassificationTwo"]
    .iter()
    .filter_map(|path| v.str(path))
    .collect();
  Some(Job {
    url: job_url(site, &id, kind),
    title: v.str("JobAdName"),
    text: joined(&[text(v, "Duty"), text(v, "Require")]),
    company,
    site: site.url(),
    cities: names(v, "LocNames", None),
    recruit: kind,
    job_type: v.str("Kind").as_deref().and_then(job_type),
    category: (!classes.is_empty()).then(|| classes.join("/")),
    // 默认部门: no department set.
    department: v.str("Org").filter(|org| org != "默认部门"),
    experience: v.str("YearsOfWorking"),
    degree: v.str("Degree"),
    deadline: date(v, "EndTime"),
    headcount: v.u64("HeadCount"),
    salary: v.str("Salary").as_deref().and_then(Salary::parse),
    created_at: v.str("PostDate").as_deref().and_then(local),
    code: v.str("JobAdId"),
    raw: v.clone(),
    id,
    ..Job::default()
  })
}
