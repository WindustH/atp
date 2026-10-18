//! What ATP reads off a job's page and a company's page
//! ([`crate::pages`]) as a post and a user.

use atp_core::model::Extra;
use atp_core::{Post, User, Value, ValueExt, json};

use super::{company_keys, salary, sanitize, text};
use crate::refs::{self, JobRef};

/// The strings of a list in a page's reading.
fn list(v: &Value, path: &str) -> Vec<String> {
  v.list(path)
    .iter()
    .filter_map(|s| s.as_str())
    .map(|s| s.trim().to_owned())
    .filter(|s| !s.is_empty())
    .collect()
}

/// A company's facts (`互联网`, `D轮及以上`, `10000人以上`) as industry,
/// stage and size.
fn facts(extra: &mut Extra, facts: &[String]) {
  for fact in facts {
    let key = if fact.ends_with('人') || fact.contains("人以") {
      "size"
    } else if fact.contains('轮') || fact.contains("融资") || fact.contains("上市") {
      "stage"
    } else {
      "industry"
    };
    extra.entry(key.to_owned()).or_insert_with(|| json!(fact));
  }
}

/// The company a job page names.
fn job_company(v: &Value) -> Option<User> {
  let name = text(v, "company")?;
  let id = text(v, "company_url")
    .as_deref()
    .and_then(|url| refs::company(url).ok());
  let mut user = User {
    url: id.as_deref().map(refs::company_url),
    avatar: text(v, "company_logo").filter(|l| l.starts_with("http")),
    id: id.unwrap_or_default(),
    name,
    ..User::default()
  };
  facts(&mut user.extra, &list(v, "company_facts"));
  Some(user)
}

/// A job's page: title, pay, place, experience, degree, the description,
/// skills and benefits, the address and the company.
pub fn job_page(v: &Value, job: &JobRef) -> Option<Post> {
  let title = text(v, "name")?;
  let author = job_company(v);
  let mut tags = list(v, "skills");
  for benefit in list(v, "welfare") {
    if !tags.contains(&benefit) {
      tags.push(benefit);
    }
  }
  let mut post = Post {
    id: job.id.clone(),
    kind: "job".into(),
    title: Some(title),
    text: text(v, "description"),
    url: Some(job.url()),
    tags,
    raw: Some(sanitize(v)),
    ..Post::default()
  };
  let extra = &mut post.extra;
  if let Some(company) = &author {
    company_keys(extra, company);
  }
  salary(extra, &v.str("salary").unwrap_or_default());
  for (key, path) in [
    ("city", "city"),
    ("experience", "experience"),
    ("degree", "degree"),
    ("address", "address"),
  ] {
    if let Some(value) = text(v, path) {
      extra.insert(key.into(), json!(value));
    }
  }
  post.author = author;
  Some(post)
}

/// A job a company's page lists.
fn company_job(v: &Value, company: &User) -> Option<Post> {
  let id = text(v, "href").and_then(|href| refs::job(&href).ok())?.id;
  let mut post = Post {
    url: Some(refs::job_url(&id)),
    kind: "job".into(),
    title: text(v, "name"),
    raw: Some(sanitize(v)),
    id,
    ..Post::default()
  };
  let extra = &mut post.extra;
  company_keys(extra, company);
  salary(extra, &v.str("salary").unwrap_or_default());
  let meta = list(v, "meta");
  for (key, value) in ["city", "experience", "degree"].iter().zip(&meta) {
    extra.insert((*key).into(), json!(value));
  }
  post.author = Some(company.clone());
  Some(post)
}

/// A company's page: name, logo, funding stage, size, industry and
/// introduction, and the jobs it lists.
pub fn company_page(v: &Value, id: &str) -> Option<(User, Vec<Post>)> {
  let name = text(v, "name")?;
  let mut user = User {
    id: id.to_owned(),
    url: Some(refs::company_url(id)),
    avatar: text(v, "logo").filter(|l| l.starts_with("http")),
    bio: text(v, "intro"),
    name,
    raw: Some(sanitize(v)),
    ..User::default()
  };
  facts(&mut user.extra, &list(v, "facts"));
  if let Some(address) = text(v, "address") {
    user.location = Some(address);
  }
  let mut short = user.clone();
  short.raw = None;
  short.bio = None;
  let jobs: Vec<Post> = v
    .list("jobs")
    .iter()
    .filter_map(|job| company_job(job, &short))
    .collect();
  if jobs.len() as u64 > 0 {
    user.stats.posts = v.u64("job_count").or(Some(jobs.len() as u64));
  }
  Some((user, jobs))
}
