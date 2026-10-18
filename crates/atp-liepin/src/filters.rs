//! The codes of the search filters, by the names the site shows: fixed
//! ones (pay, experience, degree, company size, funding stage, ownership)
//! as option values, cities and industries looked up from the search
//! page's own lists (`searchfront4c.pc-search-job-cond-init`, kept a day)
//! and, for cities beyond its hot list, the site's city suggestions
//! (`bd.p.suggest-dq`).

use std::time::Duration;

use atp_core::{Collection, Error, Result, Value, ValueExt};

use crate::api::Api;

/// The code of "all of China".
pub const NATIONWIDE: &str = "410";

const CACHE: &str = "liepin-filters";
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);

/// `--salary`: monthly pay bands.
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
  /// 10k-20k
  #[value(name = "10-20k")]
  From10k,
  /// 20k-40k
  #[value(name = "20-40k")]
  From20k,
  /// 40k-60k
  #[value(name = "40-60k")]
  From40k,
  /// 60k and more (60K以上)
  #[value(name = "60k+")]
  Above60k,
}

impl Salary {
  pub fn code(self) -> &'static str {
    match self {
      Self::Below3k => "0$3",
      Self::From3k => "3$5",
      Self::From5k => "5$10",
      Self::From10k => "10$20",
      Self::From20k => "20$40",
      Self::From40k => "40$60",
      Self::Above60k => "60$999",
    }
  }
}

/// `--experience`: years of work asked for.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Experience {
  /// New graduates (应届生)
  #[value(alias = "应届生")]
  Graduate,
  /// Interns (实习生)
  #[value(alias = "实习生")]
  Intern,
  /// Less than a year (1年以内)
  #[value(name = "0-1")]
  UpTo1,
  /// 1-3 years
  #[value(name = "1-3")]
  From1,
  /// 3-5 years
  #[value(name = "3-5")]
  From3,
  /// 5-10 years
  #[value(name = "5-10")]
  From5,
  /// More than 10 years (10年以上)
  #[value(name = "10+")]
  Above10,
}

impl Experience {
  pub fn code(self) -> &'static str {
    match self {
      Self::Graduate => "1",
      Self::Intern => "2",
      Self::UpTo1 => "0$1",
      Self::From1 => "1$3",
      Self::From3 => "3$5",
      Self::From5 => "5$10",
      Self::Above10 => "10$999",
    }
  }
}

/// `--degree`: the education asked for.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Degree {
  /// 博士
  #[value(alias = "博士")]
  Phd,
  /// 硕士
  #[value(alias = "硕士")]
  Master,
  /// 本科
  #[value(alias = "本科")]
  Bachelor,
  /// 大专
  #[value(alias = "大专")]
  College,
  /// 中专/中技
  #[value(alias = "中专")]
  Technical,
  /// 高中
  #[value(alias = "高中")]
  HighSchool,
  /// 初中及以下
  #[value(alias = "初中")]
  Middle,
}

impl Degree {
  pub fn code(self) -> &'static str {
    match self {
      Self::Phd => "010",
      Self::Master => "030",
      Self::Bachelor => "040",
      Self::College => "050",
      Self::Technical => "060",
      Self::HighSchool => "080",
      Self::Middle => "090",
    }
  }
}

/// `--size`: people working at the company.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Size {
  /// 1-49
  #[value(name = "1-49")]
  Tiny,
  /// 50-99
  #[value(name = "50-99")]
  Small,
  /// 100-499
  #[value(name = "100-499")]
  Medium,
  /// 500-999
  #[value(name = "500-999")]
  Large,
  /// 1000-2000
  #[value(name = "1000-2000")]
  From1000,
  /// 2000-5000
  #[value(name = "2000-5000")]
  From2000,
  /// 5000-10000
  #[value(name = "5000-10000")]
  From5000,
  /// More than 10000 (10000人以上)
  #[value(name = "10000+")]
  Above10000,
}

impl Size {
  pub fn code(self) -> &'static str {
    match self {
      Self::Tiny => "010",
      Self::Small => "020",
      Self::Medium => "030",
      Self::Large => "040",
      Self::From1000 => "050",
      Self::From2000 => "060",
      Self::From5000 => "070",
      Self::Above10000 => "080",
    }
  }
}

/// `--stage`: the company's funding (融资阶段).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Stage {
  /// 天使轮
  Angel,
  /// A轮
  A,
  /// B轮
  B,
  /// C轮
  C,
  /// D轮及以上
  #[value(name = "d+")]
  DPlus,
  /// 已上市
  Listed,
  /// 战略融资
  Strategic,
  /// 融资未公开
  Undisclosed,
}

impl Stage {
  pub fn code(self) -> &'static str {
    match self {
      Self::Angel => "01",
      Self::A => "02",
      Self::B => "03",
      Self::C => "04",
      Self::DPlus => "05",
      Self::Listed => "06",
      Self::Strategic => "07",
      Self::Undisclosed => "08",
    }
  }
}

/// `--kind`: who posted the job.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum JobKind {
  /// Jobs headhunters post for their clients (猎头职位)
  Headhunter,
  /// Jobs companies post themselves (企业职位)
  Company,
}

impl JobKind {
  pub fn code(self) -> &'static str {
    match self {
      Self::Headhunter => "1",
      Self::Company => "2",
    }
  }
}

/// `search --filter`: who posted the job.
pub const SEARCH_FILTERS: &[&str] = &["headhunter", "company"];

/// The search page's lists: hot cities, industries and the fixed bands.
async fn lists(api: &Api) -> Result<Value> {
  if let Some(v) = api.ctx.store.cache_get::<Value>(CACHE, CACHE_TTL) {
    return Ok(v);
  }
  let form = [("selectedDqCode", NATIONWIDE.to_owned())];
  let v = api
    .form("searchfront4c.pc-search-job-cond-init", &form)
    .await?;
  if !v.list("hotCities").is_empty() {
    api.ctx.store.cache_put(CACHE, &v);
  }
  Ok(v)
}

/// A city, with the province it is in when the site names it.
fn place(v: &Value) -> Collection {
  Collection {
    id: v.str("code").unwrap_or_default(),
    kind: "city".into(),
    name: v.str("name").unwrap_or_default(),
    description: v.str("parentName"),
    ..Collection::default()
  }
}

/// The hot cities of the search page (全国 first).
pub async fn hot_cities(api: &Api) -> Result<Vec<Collection>> {
  let v = lists(api).await?;
  Ok(v.list("hotCities").iter().map(place).collect())
}

/// Cities, provinces and districts whose names match `name`.
pub async fn find_cities(api: &Api, name: &str) -> Result<Vec<Collection>> {
  let form = [
    ("keywords", name.to_owned()),
    ("size", "20".to_owned()),
    ("sc", "0".to_owned()),
    ("lang", "zh".to_owned()),
    ("countryCategories", "-1,0,1,2,3,4".to_owned()),
    ("overseasCategories", "0,1,2,3".to_owned()),
    ("from", "component".to_owned()),
  ];
  let v = api.dok("bd.p.suggest-dq", &form).await?;
  Ok(v.list("list").iter().map(place).collect())
}

/// The code of `--city`: a code as it is, else a hot city's name, else
/// the site's best match for the name.
pub async fn city(api: &Api, arg: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  let hot = hot_cities(api).await?;
  if let Some(city) = hot.iter().find(|c| c.name == arg) {
    return Ok(city.id.clone());
  }
  let found = find_cities(api, arg).await?;
  found
    .iter()
    .find(|c| c.name == arg)
    .or_else(|| found.first())
    .map(|c| c.id.clone())
    .ok_or_else(|| {
      Error::input(format!(
        "Liepin knows no city {arg} (`atp liepin cities NAME` looks them up)"
      ))
    })
}

/// The industries of the search page, each with its group.
pub async fn industries(api: &Api) -> Result<Vec<Collection>> {
  let v = lists(api).await?;
  let mut out = Vec::new();
  for group in v.list("industries") {
    let parent = group.str("code").unwrap_or_default();
    let name = group.str("name").unwrap_or_default();
    for item in group.list("children") {
      let code = item.str("code").unwrap_or_default();
      // The group's own entry (不限) stands for the whole group.
      let whole = code == parent;
      out.push(Collection {
        id: if whole {
          parent.clone()
        } else {
          format!("{parent}${code}")
        },
        kind: if whole { "industry group" } else { "industry" }.into(),
        name: if whole {
          name.clone()
        } else {
          item.str("name").unwrap_or_default()
        },
        description: (!whole).then(|| name.clone()),
        ..Collection::default()
      });
    }
  }
  Ok(out)
}

/// The code of `--industry`: a code as `industries` lists it (`H01`,
/// `H01$H0009`), or an industry's or a group's name.
pub async fn industry(api: &Api, arg: &str) -> Result<String> {
  let arg = arg.trim();
  let all = industries(api).await?;
  all
    .iter()
    .find(|i| i.id.eq_ignore_ascii_case(arg))
    .or_else(|| all.iter().find(|i| i.name == arg))
    .map(|i| i.id.clone())
    .ok_or_else(|| {
      Error::input(format!(
        "Liepin has no industry {arg} (`atp liepin industries` lists them)"
      ))
    })
}
