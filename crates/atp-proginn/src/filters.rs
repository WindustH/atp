//! The board's filters (`recruit/get_options`, kept a day): remote or
//! on-site work, cities (eight hot ones, 413 in all) with their
//! provinces, and the roles (职业角色) with their directions. `--city`
//! and `--role` take names, the words of the board's links (`beijing`,
//! `rust`) or the ids `cities` and `roles` list.

use std::collections::BTreeMap;
use std::time::Duration;

use atp_core::{Collection, Error, Result, Value, ValueExt, json};

use crate::api::Api;

const CACHE: &str = "proginn-options";
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);

/// Where the work is done.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum WorkType {
  /// Remote work (远程)
  Remote,
  /// On-site work at the client's (驻场)
  Onsite,
}

impl WorkType {
  /// The board's `workType`.
  pub fn code(self) -> &'static str {
    match self {
      Self::Remote => "1",
      Self::Onsite => "2",
    }
  }

  /// `remote` or `onsite`, as `search --filter` and `hot --category` take them.
  pub fn named(name: Option<&str>) -> Option<Self> {
    match name {
      Some("remote") => Some(Self::Remote),
      Some("onsite") => Some(Self::Onsite),
      _ => None,
    }
  }
}

/// The values of `search --filter` and `hot --category`.
pub const WORK_TYPES: &[&str] = &["remote", "onsite"];

/// A city: the board's id and the name its jobs give.
#[derive(Debug, Clone, Default)]
pub struct City {
  pub id: String,
  pub name: String,
}

/// A role, and the direction within it.
#[derive(Debug, Clone, Default)]
pub struct Role {
  /// `occupationId`.
  pub occupation: String,
  /// `directionId`.
  pub direction: Option<String>,
}

/// The board's lists of cities and roles.
async fn options(api: &Api) -> Result<Value> {
  if let Some(v) = api.ctx.store.cache_get::<Value>(CACHE, CACHE_TTL) {
    return Ok(v);
  }
  let v = api.get("recruit/get_options").await?;
  if !v.list("cities").is_empty() {
    api.ctx.store.cache_put(CACHE, &v);
  }
  Ok(v)
}

/// A name as the site writes it, without the spaces some carry.
fn name(v: &Value, path: &str) -> String {
  v.str(path).unwrap_or_default().trim().to_owned()
}

/// The hot cities, or those whose name, link word or province matches.
pub async fn cities(api: &Api, query: Option<&str>) -> Result<Vec<Collection>> {
  let v = options(api).await?;
  let provinces: BTreeMap<String, String> = v
    .list("provinces")
    .iter()
    .filter_map(|p| Some((p.str("id")?, name(p, "name"))))
    .collect();
  let place = |c: &Value| {
    let mut city = Collection {
      id: c.str("id").unwrap_or_default(),
      kind: "city".into(),
      name: name(c, "name"),
      description: c.str("prov_id").and_then(|id| provinces.get(&id).cloned()),
      ..Collection::default()
    };
    if let Some(slug) = c.str("slug") {
      city.extra.insert("slug".into(), json!(slug));
    }
    city
  };
  let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) else {
    return Ok(v.list("hotCities").iter().map(place).collect());
  };
  let all = v.list("cities").iter().map(place);
  Ok(all.filter(|c| matches_city(c, query)).collect())
}

fn matches_city(city: &Collection, query: &str) -> bool {
  let slug = city.extra.get("slug").and_then(|s| s.as_str());
  city.name.contains(query)
    || slug.is_some_and(|s| s.eq_ignore_ascii_case(query))
    || city.description.as_deref() == Some(query)
}

/// `--city`: a city by its id, its name or its link word.
pub async fn city(api: &Api, arg: &str) -> Result<City> {
  let arg = arg.trim();
  let v = options(api).await?;
  let same = |c: &Value| {
    let slug = c.str("slug").filter(|s| !s.is_empty());
    c.str("id").as_deref() == Some(arg)
      || name(c, "name") == arg
      || slug.is_some_and(|s| s.eq_ignore_ascii_case(arg))
  };
  let found = v.list("cities").iter().find(|c| same(c)).ok_or_else(|| {
    Error::input(format!(
      "Proginn has no city {arg} (`atp proginn cities NAME` looks them up)"
    ))
  })?;
  Ok(City {
    id: found.str("id").unwrap_or_default(),
    name: name(found, "name"),
  })
}

/// Every role (`ROLE`) and direction within one (`ROLE/DIRECTION`).
pub async fn roles(api: &Api) -> Result<Vec<Collection>> {
  let v = options(api).await?;
  let mut out = Vec::new();
  for role in v.list("direction") {
    let id = role.str("occupation_id").unwrap_or_default();
    let role_name = name(role, "occupation_name");
    out.push(entry(id.clone(), "role", role_name.clone(), None, role));
    for direction in role.list("children") {
      let child = direction.str("direction_id").unwrap_or_default();
      let label = name(direction, "direction_name");
      let parent = Some(role_name.clone());
      out.push(entry(
        format!("{id}/{child}"),
        "direction",
        label,
        parent,
        direction,
      ));
    }
  }
  Ok(out)
}

fn entry(id: String, kind: &str, name: String, parent: Option<String>, v: &Value) -> Collection {
  let mut entry = Collection {
    id,
    kind: kind.into(),
    name,
    description: parent,
    ..Collection::default()
  };
  if let Some(slug) = v.str("slug") {
    entry.extra.insert("slug".into(), json!(slug));
  }
  entry
}

/// `--role`: an id as `roles` lists it, or a role's or a direction's name
/// or link word (roles first: 前端, 测试 … are both).
pub async fn role(api: &Api, arg: &str) -> Result<Role> {
  let arg = arg.trim();
  let all = roles(api).await?;
  let same = |c: &Collection| {
    let slug = c
      .extra
      .get("slug")
      .and_then(|s| s.as_str())
      .unwrap_or_default();
    c.id == arg || c.name.eq_ignore_ascii_case(arg) || slug.eq_ignore_ascii_case(arg)
  };
  let found = all
    .iter()
    .filter(|c| c.kind == "role")
    .find(|c| same(c))
    .or_else(|| all.iter().find(|c| same(c)))
    .ok_or_else(|| {
      Error::input(format!(
        "Proginn has no role {arg} (`atp proginn roles` lists them)"
      ))
    })?;
  let mut ids = found.id.splitn(2, '/');
  Ok(Role {
    occupation: ids.next().unwrap_or_default().to_owned(),
    direction: ids.next().map(str::to_owned),
  })
}
