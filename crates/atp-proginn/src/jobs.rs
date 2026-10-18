//! Jobs: the board's search (`recruit/search`, newest first, 20 a page,
//! pages counted from 1), which `search`, `hot` and `jobs` all page
//! through, and one job (`recruit/getRecruit`).

use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt};

use crate::api::{Api, NEEDS_LOGIN, message};
use crate::filters::{City, Role, WorkType};
use crate::{parse, refs};

/// Jobs a page of the search holds.
const PAGE: u64 = 20;

/// What a search is narrowed to.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  pub keyword: String,
  pub work_type: Option<WorkType>,
  /// A city; every city by default.
  pub city: Option<City>,
  pub role: Option<Role>,
}

/// `search WORDS`, `--filter remote|onsite`.
pub async fn search(api: &Api, query: &Query, req: &PageReq) -> Result<Page<Post>> {
  let filter = Filter {
    keyword: query.keyword.trim().to_owned(),
    work_type: WorkType::named(query.filter.as_deref()),
    ..Filter::default()
  };
  list(api, &filter, req).await
}

/// `hot`: every open job, newest first, `--category remote|onsite`.
pub async fn hot(api: &Api, category: Option<&str>, req: &PageReq) -> Result<Page<Post>> {
  let filter = Filter {
    work_type: WorkType::named(category),
    ..Filter::default()
  };
  list(api, &filter, req).await
}

/// One page of the board's search, with the fields its job page sends.
/// The board takes the city along but still lists jobs elsewhere, so the
/// jobs of other cities (and remote ones, which name none) are left out
/// here.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1).max(1);
  let city = filter.city.as_ref();
  let mut form = vec![
    ("city", city.map_or_else(|| "0".into(), |c| c.id.clone())),
    ("page", page.to_string()),
    ("pageSize", PAGE.to_string()),
    ("total", "0".into()),
    ("keyword", filter.keyword.clone()),
  ];
  if let Some(role) = &filter.role {
    form.push(("occupationId", role.occupation.clone()));
    if let Some(direction) = &role.direction {
      form.push(("directionId", direction.clone()));
    }
  }
  if let Some(work_type) = filter.work_type {
    form.push(("workType", work_type.code().into()));
  }
  let v = api.post("recruit/search", &form).await?;
  let records = v.list("list");
  let pages = v.u64("pages").unwrap_or(0);
  let next = (!records.is_empty() && page < pages).then(|| (page + 1).to_string());
  let mut items: Vec<Post> = records.iter().filter_map(parse::job).collect();
  if let Some(city) = city {
    items.retain(|post| post.extra.get("city").and_then(Value::as_str) == Some(&city.name));
  }
  Ok(Page::new(items, next))
}

/// A job as `getRecruit` answers it (with a login, also what the account
/// did with it). A job taken down, or never there, is answered with a
/// status other than 1 and words such as 项目已下线.
pub async fn recruit(api: &Api, arg: &str) -> Result<Value> {
  let id = refs::job(arg)?;
  let answer = api
    .answer("recruit/getRecruit", &[("recruitId", id.clone())])
    .await?;
  let gone = |why: String| Error::not_found(format!("Proginn shows no job {id}: {why}"));
  if answer
    .i64("status")
    .is_some_and(|s| s != 1 && s != NEEDS_LOGIN)
  {
    return Err(gone(message(&answer)));
  }
  let v = api.data(answer)?;
  if v.str("id").is_none() && v.str("recruitId").is_none() {
    return Err(gone("the answer names no job".into()));
  }
  Ok(v)
}

/// A job's page.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let v = recruit(api, arg).await?;
  parse::job(&v).ok_or_else(|| Error::upstream("Proginn's job answer had no id"))
}
