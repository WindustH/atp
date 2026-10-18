//! The codes of the filters, by the names the site shows: fixed ones (job
//! kinds, pay, company ownership and size, the schedule's tabs) as option
//! values; career categories (`/api/sparta/all-job`, a tree of three
//! levels) and what the 校招 schedule filters by (`/u/school-schedule/
//! support`: industries, company types, batches), looked up by name and
//! kept a day.

use std::time::Duration;

use atp_core::{Collection, Error, Result, Value, ValueExt, json};

use crate::api::{Api, Route};

const CAREERS: &str = "nowcoder-careers";
const SUPPORT: &str = "nowcoder-schedule-support";
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);

/// `jobs --kind` and `search --filter`: 校招, 实习, 社招.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum JobKind {
  /// 校招: campus hiring of new graduates (the default)
  #[value(alias = "校招")]
  Campus,
  /// 实习: internships
  #[value(alias = "实习")]
  Intern,
  /// 社招: experienced hires
  #[value(alias = "社招")]
  Social,
}

impl JobKind {
  /// `recruitType`.
  pub fn code(self) -> u8 {
    match self {
      Self::Campus => 1,
      Self::Intern => 2,
      Self::Social => 3,
    }
  }
}

/// `jobs --salary`: monthly pay of campus and experienced jobs.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Salary {
  /// Below 3k a month (3K以下)
  #[value(name = "0-3k")]
  Below3k,
  /// 3k-5k
  #[value(name = "3-5k")]
  From3k,
  /// 5k-10k
  #[value(name = "5-10k")]
  From5k,
  /// 10k-15k
  #[value(name = "10-15k")]
  From10k,
  /// 15k-20k
  #[value(name = "15-20k")]
  From15k,
  /// 20k-30k
  #[value(name = "20-30k")]
  From20k,
  /// 30k-50k
  #[value(name = "30-50k")]
  From30k,
  /// 50k and more (50K以上)
  #[value(name = "50k+")]
  Above50k,
}

impl Salary {
  /// `salaryMin` and `salaryMax`, in thousands a month.
  pub fn range(self) -> (u32, u32) {
    match self {
      Self::Below3k => (0, 3),
      Self::From3k => (3, 5),
      Self::From5k => (5, 10),
      Self::From10k => (10, 15),
      Self::From15k => (15, 20),
      Self::From20k => (20, 30),
      Self::From30k => (30, 50),
      Self::Above50k => (50, 300),
    }
  }
}

/// `jobs --nature`: who owns the company (企业性质).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Nature {
  /// 国企: state-owned
  #[value(alias = "国企")]
  State,
  /// 央企: owned by the central government
  #[value(alias = "央企")]
  Central,
  /// 银行/信用社: banks and credit unions
  #[value(alias = "银行")]
  Bank,
  /// 研究所/院: research institutes
  #[value(alias = "研究所")]
  Institute,
  /// 外企: foreign companies
  #[value(alias = "外企")]
  Foreign,
  /// 民企: private companies
  #[value(alias = "民企")]
  Private,
  /// 事业单位: public institutions
  #[value(alias = "事业单位")]
  Public,
}

impl Nature {
  /// `companyNature`.
  pub fn code(self) -> u32 {
    match self {
      Self::State => 2830,
      Self::Central => 274572,
      Self::Bank => 274571,
      Self::Institute => 274574,
      Self::Foreign => 2834,
      Self::Private => 4712,
      Self::Public => 4713,
    }
  }
}

/// `jobs --size`: people at the company.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Size {
  #[value(name = "0-20")]
  UpTo20,
  #[value(name = "20-99")]
  From20,
  #[value(name = "100-499")]
  From100,
  #[value(name = "500-999")]
  From500,
  #[value(name = "1000-9999")]
  From1000,
  #[value(name = "10000+")]
  From10000,
}

impl Size {
  /// `scaleList`.
  pub fn code(self) -> u8 {
    match self {
      Self::UpTo20 => 1,
      Self::From20 => 2,
      Self::From100 => 3,
      Self::From500 => 4,
      Self::From1000 => 5,
      Self::From10000 => 6,
    }
  }
}

/// `schedule --tab`: the schedule page's tabs.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Tab {
  /// 推荐: the site's order (the default)
  Recommended,
  /// 即将截止: applications closing soon
  Closing,
  /// 最近更新: newly added or changed
  Updated,
  /// 补招: supplementary hiring
  Supplementary,
  /// 专场: special events
  Special,
  /// 我的收藏: the companies you subscribed to (needs a login)
  Subscribed,
}

impl Tab {
  /// `tab`.
  pub fn code(self) -> u8 {
    match self {
      Self::Recommended => 1,
      Self::Closing => 2,
      Self::Updated => 3,
      Self::Supplementary => 4,
      Self::Subscribed => 5,
      Self::Special => 6,
    }
  }
}

/// A value of the cache, or of `fetch` (kept when it is not empty).
async fn cached(api: &Api, key: &str, fetch: impl AsyncFnOnce() -> Result<Value>) -> Result<Value> {
  let store = &api.ctx.store;
  if let Some(v) = store.cache_get::<Value>(key, CACHE_TTL) {
    return Ok(v);
  }
  let v = fetch().await?;
  if !v.is_null() {
    store.cache_put(key, &v);
  }
  Ok(v)
}

/// A career category: id, name, level (1 to 3), the path down to it and
/// the category it belongs to.
#[derive(Debug, Clone)]
pub struct Career {
  pub id: String,
  pub name: String,
  pub level: u64,
  pub path: String,
  pub parent: Option<String>,
}

fn flatten(nodes: &[Value], above: Option<&Career>, out: &mut Vec<Career>) {
  for node in nodes {
    let info = node.at("jobInfo");
    let (Some(id), Some(name)) = (info.str("id"), info.str("name")) else {
      continue;
    };
    let career = Career {
      level: info.u64("level").unwrap_or(1),
      path: above.map_or(name.clone(), |a| format!("{} / {name}", a.path)),
      parent: above.map(|a| a.id.clone()),
      id,
      name,
    };
    out.push(career.clone());
    flatten(node.list("subJobs"), Some(&career), out);
  }
}

/// Every career category, the tree flattened in order.
pub async fn all_careers(api: &Api) -> Result<Vec<Career>> {
  let tree = cached(api, CAREERS, async || {
    let v = api.get(Route::Gateway, "/api/sparta/all-job", &[]).await?;
    Ok(v.at("result").clone())
  })
  .await?;
  let mut out = Vec::new();
  flatten(tree.as_array().map_or(&[], Vec::as_slice), None, &mut out);
  Ok(out)
}

/// The career categories as collections, those whose path names `name`
/// when given.
pub async fn careers(api: &Api, name: Option<&str>) -> Result<Vec<Collection>> {
  let wanted = name.map(str::to_lowercase);
  Ok(
    all_careers(api)
      .await?
      .into_iter()
      .filter(|c| {
        wanted
          .as_deref()
          .is_none_or(|w| c.path.to_lowercase().contains(w))
      })
      .map(|c| {
        let mut item = Collection {
          id: c.id,
          kind: "career".into(),
          name: c.name,
          description: Some(c.path),
          ..Collection::default()
        };
        item.extra.insert("level".into(), json!(c.level));
        item
      })
      .collect(),
  )
}

/// A career category by its id or name (`Java`, `后端开发`, `软件开发`), at
/// `level` when the filter takes one level only: the job search takes the
/// third (Java), the schedule the second (后端开发). A narrower category
/// stands for the one above it at that level; a broader one fails with
/// the categories under it.
pub async fn career(api: &Api, value: &str, level: Option<u64>) -> Result<Career> {
  let value = value.trim();
  let all = all_careers(api).await?;
  let lower = value.to_lowercase();
  let exact = |c: &&Career| c.id == value || c.name.to_lowercase() == lower;
  let close = |c: &&Career| c.name.to_lowercase().contains(&lower);
  let at = |c: &&Career| level.is_none_or(|l| c.level == l);
  if let Some(found) = all.iter().find(|c| at(c) && exact(c)) {
    return Ok(found.clone());
  }
  // A part of a name finds the first category in the tree's order.
  let Some(found) = all.iter().find(exact).or_else(|| all.iter().find(close)) else {
    return Err(Error::input(format!(
      "Nowcoder has no career category {value} (`careers` lists them)"
    )));
  };
  let Some(level) = level else {
    return Ok(found.clone());
  };
  // Narrower: the category above it at the level.
  let mut up = found;
  while up.level > level {
    match up
      .parent
      .as_ref()
      .and_then(|p| all.iter().find(|c| &c.id == p))
    {
      Some(parent) => up = parent,
      None => break,
    }
  }
  if up.level == level {
    return Ok(up.clone());
  }
  let under: Vec<&str> = all
    .iter()
    .filter(|c| c.level == level && c.path.starts_with(&format!("{} / ", found.path)))
    .map(|c| c.name.as_str())
    .take(40)
    .collect();
  Err(Error::input(format!(
    "{} is a broad category here; --career takes one of: {}",
    found.name,
    under.join(", ")
  )))
}

/// What the schedule filters by.
async fn support(api: &Api) -> Result<Value> {
  cached(api, SUPPORT, async || {
    api
      .get(Route::Jobs, "/u/school-schedule/support", &[])
      .await
  })
  .await
}

/// The id of the entry of `list` named `value` (or with that id).
async fn pick(api: &Api, list: &str, value: &str, what: &str) -> Result<String> {
  let v = support(api).await?;
  let items = v.list(list);
  let value = value.trim();
  let lower = value.to_lowercase();
  let name = |i: &Value| i.str("name").unwrap_or_default().to_lowercase();
  let found = items
    .iter()
    .find(|i| i.str("id").as_deref() == Some(value) || name(i) == lower)
    .or_else(|| items.iter().find(|i| name(i).contains(&lower)));
  if let Some(id) = found.and_then(|i| i.str("id")) {
    return Ok(id);
  }
  let names: Vec<String> = items.iter().filter_map(|i| i.str("name")).collect();
  Err(Error::input(format!(
    "Nowcoder has no such {what}: {value} (one of: {})",
    names.join(", ")
  )))
}

/// `schedule --batch`: 27届秋招, 日常实习 … or the batch's id.
pub async fn batch(api: &Api, value: &str) -> Result<String> {
  pick(api, "batchList", value, "batch").await
}

/// `schedule --industry`: 人工智能, 银行 … or the industry's id.
pub async fn industry(api: &Api, value: &str) -> Result<String> {
  pick(api, "industryList", value, "industry").await
}

/// `schedule --type`: 国企, 外企, 民企, 事业单位 or the type's id.
pub async fn company_type(api: &Api, value: &str) -> Result<String> {
  pick(api, "companyPropertyList", value, "company type").await
}
