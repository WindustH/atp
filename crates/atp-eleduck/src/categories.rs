//! Categories (版块) and their tags (`/categories?show_paid=true`, which
//! carries every category's tag groups), kept a day in the cache. The tags
//! of the jobs channel's category filter `jobs`.

use std::time::Duration;

use atp_core::{Collection, Error, Result, Value, ValueExt};

use crate::api::Api;
use crate::parse;

const CACHE_KEY: &str = "eleduck-categories";
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
/// 简历智能匹配: the category whose tag groups the jobs channel filters by.
const JOBS_CHANNEL: u64 = 32;
/// 社区帖子招聘, the older job category with the same tag groups.
const JOB_POSTS: u64 = 5;

/// English words `jobs --type` takes for the 工作方式 tags.
const WORK_MODES: &[(&str, &str)] = &[
  ("remote", "全职远程"),
  ("onsite", "全职坐班"),
  ("part-time", "线上兼职"),
  ("offline-part-time", "线下兼职"),
  ("hybrid", "混合办公"),
];

/// Every category, from the cache or the API.
async fn all(api: &Api) -> Result<Vec<Value>> {
  let store = &api.ctx.store;
  if let Some(Value::Array(list)) = store.cache_get::<Value>(CACHE_KEY, CACHE_TTL) {
    return Ok(list);
  }
  let v = api
    .get("/categories", &[("show_paid", "true".to_owned())])
    .await?;
  let list = v.list("categories").to_vec();
  if !list.is_empty() {
    store.cache_put(CACHE_KEY, &Value::Array(list.clone()));
  }
  Ok(list)
}

pub async fn list(api: &Api) -> Result<Vec<Collection>> {
  Ok(all(api).await?.iter().map(parse::category).collect())
}

async fn find(api: &Api, id: u64) -> Result<Value> {
  all(api)
    .await?
    .into_iter()
    .find(|c| c.u64("id") == Some(id))
    .ok_or_else(|| Error::not_found(format!("Eleduck has no category {id} (see `categories`)")))
}

/// The option of `jobs` that filters by each tag group.
const GROUP_OPTIONS: &[(&str, &str)] = &[
  ("job_type", "--type"),
  ("hire_type", "--hire"),
  ("skill_type", "--role"),
  ("city", "--city"),
  ("field", "--field"),
];

/// The tags of a category, the jobs channel's by default, each described
/// by its group and the `jobs` option that takes it.
pub async fn tags(api: &Api, id: Option<u64>) -> Result<Vec<Collection>> {
  let mut tags = parse::tags(&find(api, id.unwrap_or(JOBS_CHANNEL)).await?);
  for tag in &mut tags {
    let group = tag
      .extra
      .get("group")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let name = tag
      .extra
      .get("group_name")
      .and_then(Value::as_str)
      .unwrap_or(group);
    let option = GROUP_OPTIONS.iter().find(|(code, _)| *code == group);
    tag.description = Some(match option {
      Some((_, option)) => format!("{name} (jobs {option})"),
      None => name.to_owned(),
    });
  }
  Ok(tags)
}

/// The jobs channel's tag groups.
async fn job_groups(api: &Api) -> Result<Value> {
  match find(api, JOBS_CHANNEL).await {
    Ok(category) => Ok(category),
    Err(_) => find(api, JOB_POSTS).await,
  }
}

/// The id of the tag `value` names in a tag group of the jobs channel:
/// its id, its name as the site shows it, or (for 工作方式) an English word.
pub async fn job_tag(api: &Api, group: &str, value: &str) -> Result<String> {
  let value = value.trim();
  let category = job_groups(api).await?;
  let tags: Vec<&Value> = category
    .list("tag_groups")
    .iter()
    .filter(|g| g.str("code").as_deref() == Some(group))
    .flat_map(|g| g.list("tags"))
    .collect();
  let wanted = WORK_MODES
    .iter()
    .find(|(word, _)| group == "job_type" && word.eq_ignore_ascii_case(value))
    .map_or(value, |(_, name)| *name);
  let found = tags.iter().find(|t| {
    t.str("id").as_deref() == Some(wanted)
      || t
        .str("name")
        .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
  });
  if let Some(id) = found.and_then(|t| t.str("id")) {
    return Ok(id);
  }
  let names: Vec<String> = tags.iter().filter_map(|t| t.str("name")).collect();
  Err(Error::input(format!(
    "Eleduck has no such tag: {value} (one of: {})",
    names.join(", ")
  )))
}
