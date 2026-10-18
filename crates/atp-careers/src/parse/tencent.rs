//! Tencent's answers: join.qq.com's projects, cities, categories and
//! jobs (search rows and job pages), careers.tencent.com's jobs.

use atp_core::{Collection, Value, ValueExt, json};

use super::{Job, day, joined, names, text};
use crate::jobs::Kind;
use crate::tencent::{CAREERS, JOIN};

const COMPANY: &str = "腾讯";

pub fn campus_url(id: &str) -> String {
  format!("{JOIN}/post_detail.html?postid={id}")
}

pub fn social_url(id: &str) -> String {
  format!("{CAREERS}/jobdesc.html?postId={id}")
}

/// A project of join.qq.com (2027校园招聘, 日常实习 …) by its mapping id.
pub struct Project {
  pub id: u64,
  /// 1 应届毕业生, 2 实习生, 999 人才专项.
  pub recruit: i64,
  pub name: String,
  pub group: Option<String>,
}

/// The projects, under their kinds of hiring.
pub fn projects(v: &Value) -> Vec<Project> {
  let mut list = Vec::new();
  for group in v.as_array().map(Vec::as_slice).unwrap_or_default() {
    for p in group.list("subProjectList") {
      let Some(id) = p.u64("mappingId").or_else(|| p.u64("id")) else {
        continue;
      };
      list.push(Project {
        id,
        recruit: p
          .i64("recruitType")
          .or_else(|| group.i64("recruitType"))
          .unwrap_or_default(),
        name: p.str("projectName").unwrap_or_else(|| id.to_string()),
        group: group.str("recruitTypeName"),
      });
    }
  }
  list
}

/// `batches`: the projects.
pub fn batches(v: &Value) -> Vec<Collection> {
  projects(v)
    .into_iter()
    .map(|p| Collection {
      id: p.id.to_string(),
      kind: "batch".into(),
      name: p.name,
      description: p.group,
      ..Collection::default()
    })
    .collect()
}

/// The cities of the filter, China's first; cities abroad say so.
pub fn cities(v: &Value) -> Vec<Collection> {
  let mut list = Vec::new();
  for (group, abroad) in [("1", false), ("2", true)] {
    for c in v.list(group) {
      if let (Some(id), Some(name)) = (c.str("code"), c.str("name")) {
        list.push(Collection {
          id,
          kind: "city".into(),
          name,
          description: abroad.then(|| "海外".to_owned()),
          ..Collection::default()
        });
      }
    }
  }
  list
}

/// The categories (职位类别) of the filter.
pub fn categories(v: &Value) -> Vec<Collection> {
  let Some(groups) = v.as_object() else {
    return Vec::new();
  };
  groups
    .values()
    .flat_map(|list| list.as_array().map(Vec::as_slice).unwrap_or_default())
    .filter_map(|c| {
      Some(Collection {
        id: c.str("id")?,
        kind: "category".into(),
        name: c.str("title")?,
        ..Collection::default()
      })
    })
    .collect()
}

/// The kind of a project's label (应届毕业生, 应届实习, 日常实习).
fn recruit(label: Option<&str>) -> Kind {
  match label {
    Some(label) if label.contains("实习") => Kind::Intern,
    _ => Kind::Campus,
  }
}

/// A row of join.qq.com's search.
pub fn campus_row(v: &Value) -> Option<Job> {
  let id = v.str("postId")?;
  let label = v.first_str(&["recruitLabelName", "projectName"]);
  let cities: Vec<String> = v
    .str("workCities")
    .unwrap_or_default()
    .split_whitespace()
    .map(str::to_owned)
    .collect();
  Some(Job {
    url: campus_url(&id),
    title: v.str("positionTitle"),
    company: COMPANY.into(),
    site: JOIN.into(),
    cities,
    recruit: Some(recruit(label.as_deref())),
    batch: label,
    tags: v
      .str("bgs")
      .unwrap_or_default()
      .split_whitespace()
      .map(str::to_owned)
      .collect(),
    raw: v.clone(),
    id,
    ..Job::default()
  })
}

/// A job page of join.qq.com: duties, requirements, what counts extra,
/// work and interview cities, the business groups that hire for it.
pub fn campus_detail(v: &Value) -> Option<Job> {
  let id = v.str("postId")?;
  let label = v.first_str(&["recruitLabelName", "projectName"]);
  let kind = match v.i64("recruitType") {
    Some(2) => Kind::Intern,
    Some(1) => Kind::Campus,
    _ => recruit(label.as_deref()),
  };
  let bonus = match kind {
    Kind::Intern => text(v, "internBonus"),
    _ => text(v, "graduateBonus"),
  };
  let groups = names(v, "intentionBGDList", Some("showTitle"));
  let mut job = Job {
    url: campus_url(&id),
    title: v.str("title"),
    text: joined(&[
      text(v, "desc"),
      text(v, "request"),
      bonus.map(|b| format!("加分项：\n{b}")),
    ]),
    company: COMPANY.into(),
    site: JOIN.into(),
    cities: names(v, "workCityList", None),
    recruit: Some(kind),
    category: v.str("tidName"),
    batch: v.first_str(&["projectName", "recruitLabelName"]),
    tags: groups,
    raw: v.clone(),
    id,
    ..Job::default()
  };
  let interviews = names(v, "recruitCityList", None);
  if !interviews.is_empty() {
    job.more.push(("interview_cities", json!(interviews)));
  }
  Some(job)
}

/// A job of careers.tencent.com, from the search or its page.
pub fn social(v: &Value) -> Option<Job> {
  let id = v.str("PostId")?;
  let department = [v.str("BGName"), v.str("ProductName")]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
  let mut job = Job {
    url: social_url(&id),
    title: v.str("RecruitPostName"),
    text: joined(&[
      text(v, "Responsibility"),
      text(v, "Requirement"),
      text(v, "ImportantItem"),
      text(v, "PostLightItem"),
    ]),
    company: COMPANY.into(),
    site: CAREERS.into(),
    cities: v.str("LocationName").into_iter().collect(),
    recruit: Some(Kind::Social),
    category: v.str("CategoryName"),
    department: (!department.is_empty()).then_some(department),
    experience: v.str("RequireWorkYearsName"),
    updated_at: v.str("LastUpdateTime").as_deref().and_then(day),
    raw: v.clone(),
    id,
    ..Job::default()
  };
  if let Some(country) = v.str("CountryName").filter(|c| c != "中国") {
    job.more.push(("country", json!(country)));
  }
  Some(job)
}
