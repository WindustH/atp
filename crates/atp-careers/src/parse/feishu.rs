//! Feishu Hire answers: the website a page describes, jobs (search rows
//! and job pages alike) and the codes of the search's filters.

use atp_core::text::from_millis;
use atp_core::{Collection, Value, ValueExt};

use super::{Job, job_type, joined, names, text};
use crate::feishu::Portal;
use crate::jobs::Kind;

/// The website a page describes (`<script id="js-websiteInfo">`): the
/// path the API takes, and the company.
pub fn website(html: &str) -> Option<(String, Option<String>)> {
  let start = html.find("id=\"js-websiteInfo\"")?;
  let body = &html[start..];
  let body = &body[body.find('>')? + 1..];
  let body = &body[..body.find("</script>")?];
  let v: Value = serde_json::from_str(body.trim()).ok()?;
  let path = v.str("website_info.path")?;
  let company = v.first_str(&["tenant_info.tenant_name", "tenant_info.name"]);
  Some((path, company))
}

/// Degrees by Feishu Hire's codes (`required_degree`).
fn degree(code: i64) -> Option<&'static str> {
  Some(match code {
    1 => "小学",
    2 => "初中",
    3 => "专职",
    4 => "高中",
    5 => "大专",
    6 => "本科",
    7 => "硕士",
    8 => "博士",
    9 => "其他",
    11 => "MBA",
    20 => "不限",
    _ => return None,
  })
}

/// Experience by Feishu Hire's codes (`experience`).
fn experience(code: i64) -> Option<&'static str> {
  Some(match code {
    1 => "不限",
    2 => "应届毕业生",
    3 => "1 年以下",
    4 => "1-3 年",
    5 => "3-5 年",
    6 => "5-7 年",
    7 => "7-10 年",
    8 => "10 年以上",
    _ => return None,
  })
}

/// The kind of hiring of a `recruit_type`: under 1 社招, under 2 校招,
/// whose 202 (实习) are internships.
fn recruit(v: &Value) -> Option<Kind> {
  let parent = v.str("parent.id").or_else(|| v.str("id"))?;
  match parent.as_str() {
    "1" => Some(Kind::Social),
    "2" if v.str("id").as_deref() == Some("202") => Some(Kind::Intern),
    "2" => Some(Kind::Campus),
    _ => None,
  }
}

/// `研发/后端` of a category and its parent.
fn category(v: &Value) -> Option<String> {
  let name = v.str("name")?;
  Some(match v.str("parent.name") {
    Some(parent) => format!("{parent}/{name}"),
    None => name,
  })
}

/// A job, from the search or a job page.
pub fn job(v: &Value, portal: &Portal) -> Option<Job> {
  let id = v.str("id")?;
  let info = v.at("job_post_info");
  let mut cities = names(v, "city_list", Some("name"));
  if cities.is_empty() {
    cities.extend(v.str("city_info.name"));
  }
  let kind = v.at("recruit_type");
  Some(Job {
    url: portal.job_url(&id),
    title: v.str("title"),
    text: joined(&[text(v, "description"), text(v, "requirement")]),
    company: portal.company.clone(),
    site: portal.site_url(),
    cities,
    recruit: recruit(kind),
    job_type: kind.str("name").as_deref().and_then(job_type),
    category: category(v.at("job_category")),
    department: v.str("department_info.name"),
    batch: v.first_str(&["job_subject.name.zh_cn", "job_subject.name.i18n"]),
    experience: info
      .i64("experience")
      .and_then(experience)
      .map(str::to_owned),
    degree: info
      .i64("required_degree")
      .and_then(degree)
      .map(str::to_owned),
    address: info.str("address"),
    headcount: info.u64("head_count"),
    code: v.str("code"),
    created_at: v.i64("publish_time").and_then(from_millis),
    raw: v.clone(),
    id,
    ..Job::default()
  })
}

/// A code and its name, as `cities`, `categories` and `batches` list them.
fn code(id: String, kind: &str, name: String, description: Option<String>) -> Collection {
  Collection {
    id,
    kind: kind.into(),
    name,
    description,
    ..Collection::default()
  }
}

/// The cities of the search's filter.
pub fn cities(v: &Value) -> Vec<Collection> {
  v.list("city_list")
    .iter()
    .filter_map(|c| Some(code(c.str("code")?, "city", c.str("name")?, None)))
    .collect()
}

/// The categories, each with the ones under it (`研发/后端`).
pub fn categories(v: &Value) -> Vec<Collection> {
  let mut list = Vec::new();
  for top in v.list("job_type_list") {
    let (Some(id), Some(name)) = (top.str("id"), top.str("name")) else {
      continue;
    };
    list.push(code(id, "category", name.clone(), None));
    for sub in top.list("children") {
      if let (Some(id), Some(sub_name)) = (sub.str("id"), sub.str("name")) {
        list.push(code(id, "category", format!("{name}/{sub_name}"), None));
      }
    }
  }
  list
}

/// The subjects (校招, 实习 projects), the site's batches.
pub fn subjects(v: &Value) -> Vec<Collection> {
  v.list("job_subject_list")
    .iter()
    .filter_map(|s| {
      let name = s.first_str(&["name.zh_cn", "name.i18n"])?;
      let group = s.str("subject_group_info.name");
      Some(code(s.str("id")?, "batch", name, group))
    })
    .collect()
}
