//! Kuaishou's answers: projects, the dictionaries behind codes, and jobs
//! (search rows and job pages alike).

use atp_core::text::from_millis;
use atp_core::{Collection, Value, ValueExt, json};

use super::{Job, joined, local, names, text};
use crate::jobs::Kind;
use crate::kuaishou::WWW;

const COMPANY: &str = "快手";

pub fn job_url(id: &str) -> String {
  format!("{WWW}/recruit/campus/e/#/campus/job-info/{id}")
}

/// The code of the newest open project of a kind (`fulltime`, `intern`).
pub fn newest(v: &Value, kind: &str) -> Option<String> {
  v.list("list")
    .iter()
    .filter(|p| p.str("projectType").as_deref() == Some(kind) && p.bool("active") != Some(false))
    .max_by_key(|p| {
      (
        p.str("year").unwrap_or_default(),
        p.i64("id").unwrap_or_default(),
      )
    })
    .and_then(|p| p.str("code"))
}

/// `batches`: the projects, newest first.
pub fn projects(v: &Value) -> Vec<Collection> {
  let mut list: Vec<&Value> = v.list("list").iter().collect();
  list.sort_by_key(|p| std::cmp::Reverse(p.i64("id").unwrap_or_default()));
  list
    .into_iter()
    .filter_map(|p| {
      let kind = match p.str("projectType").as_deref() {
        Some("intern") => "实习",
        _ => "校招",
      };
      let graduates = match (p.str("graduateStartTime"), p.str("graduateEndTime")) {
        (Some(from), Some(to)) => format!("，毕业于 {from} – {to}"),
        _ => String::new(),
      };
      Some(Collection {
        id: p.str("code")?,
        kind: "batch".into(),
        name: p.str("name")?,
        description: Some(format!("{kind}{graduates}")),
        ..Collection::default()
      })
    })
    .collect()
}

/// The cities of the filter.
pub fn cities(dict: &Value) -> Vec<Collection> {
  dict
    .list("workLocation")
    .iter()
    .filter(|c| c.bool("ifActive") != Some(false))
    .filter_map(|c| {
      Some(Collection {
        id: c.str("code")?,
        kind: "city".into(),
        name: c.str("name")?,
        ..Collection::default()
      })
    })
    .collect()
}

/// The categories, each with the ones under it (`工程类/服务端`).
pub fn categories(dict: &Value) -> Vec<Collection> {
  let mut list = Vec::new();
  for top in dict.list("positionCategory") {
    if top.bool("ifActive") == Some(false) {
      continue;
    }
    let (Some(id), Some(name)) = (top.str("code"), top.str("name")) else {
      continue;
    };
    list.push(category(id, name.clone()));
    for sub in top.list("children") {
      if let (Some(id), Some(sub_name)) = (sub.str("code"), sub.str("name")) {
        list.push(category(id, format!("{name}/{sub_name}")));
      }
    }
  }
  list
}

fn category(id: String, name: String) -> Collection {
  Collection {
    id,
    kind: "category".into(),
    name,
    ..Collection::default()
  }
}

/// A category's code with those of the categories under it, as the
/// pages send a category.
pub fn with_children(dict: &Value, code: &str) -> Vec<String> {
  let mut codes = vec![code.to_owned()];
  for top in dict.list("positionCategory") {
    if top.str("code").as_deref() == Some(code) {
      codes.extend(top.list("children").iter().filter_map(|c| c.str("code")));
    }
  }
  codes
}

/// What names the codes of a job: the dictionaries and the projects.
#[derive(Debug, Default)]
pub struct Names {
  pub dict: Value,
  pub projects: Value,
}

impl Names {
  fn category(&self, code: &str) -> Option<String> {
    categories(&self.dict)
      .into_iter()
      .find(|c| c.id == code)
      .map(|c| c.name)
  }

  fn project(&self, code: &str) -> Option<String> {
    self
      .projects
      .list("list")
      .iter()
      .find(|p| p.str("code").as_deref() == Some(code))
      .and_then(|p| p.str("name"))
  }
}

/// A job, from the search or its page.
pub fn job(v: &Value, known: &Names) -> Option<Job> {
  let id = v.str("id")?;
  let kind = match v.str("positionNatureCode").as_deref() {
    Some("intern") => Kind::Intern,
    _ => Kind::Campus,
  };
  let mut job = Job {
    url: job_url(&id),
    title: v.str("name"),
    text: joined(&[text(v, "description"), text(v, "positionDemand")]),
    company: COMPANY.into(),
    site: format!("{WWW}/recruit/campus/e/"),
    cities: names(v, "workLocationDicts", Some("name")),
    recruit: Some(kind),
    category: v
      .str("positionCategoryCode")
      .map(|code| known.category(&code).unwrap_or(code)),
    batch: v
      .str("recruitSubProjectCode")
      .map(|code| known.project(&code).unwrap_or(code)),
    created_at: v.str("releaseTime").as_deref().and_then(local),
    updated_at: v.i64("updateTime").and_then(from_millis),
    raw: v.clone(),
    id,
    ..Job::default()
  };
  if let Some(n) = v.u64("applyNum").filter(|n| *n > 0) {
    job.more.push(("applicants", json!(n)));
  }
  Some(job)
}
