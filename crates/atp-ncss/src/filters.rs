//! The codes of the job search's filters, by the names the site shows:
//! fixed ones (job type, kind, degree, pay, company ownership, source) as
//! option values; areas (provinces from the page's tables, cities and
//! districts from `/student/code?type=areacodejc`, kept a week), job
//! categories and industries looked up by name.

use std::time::Duration;

use atp_core::{Collection, Error, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::codes::{CATEGORIES, INDUSTRIES, INDUSTRY_GROUPS, PROVINCES};

const AREAS: &str = "ncss-areas";
const AREAS_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

/// `--type`: full-time, part-time or internship (`jobType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum JobType {
  /// 全职
  #[value(alias = "全职")]
  FullTime,
  /// 兼职
  #[value(alias = "兼职")]
  PartTime,
  /// 实习
  #[value(alias = "实习")]
  Intern,
}

impl JobType {
  pub fn code(self) -> &'static str {
    match self {
      Self::FullTime => "01",
      Self::PartTime => "02",
      Self::Intern => "03",
    }
  }

  /// `job_type` of the shared job keys.
  pub fn name(self) -> &'static str {
    match self {
      Self::FullTime => "full-time",
      Self::PartTime => "part-time",
      Self::Intern => "intern",
    }
  }

  /// The type `search --filter` names.
  pub fn from_name(name: &str) -> Option<Self> {
    [Self::FullTime, Self::PartTime, Self::Intern]
      .into_iter()
      .find(|t| t.name() == name)
  }

  /// The type of a job page's `[全职]` or a card's `jobType` code.
  pub fn from_site(text: &str) -> Option<Self> {
    Some(match text.trim() {
      "01" | "全职" => Self::FullTime,
      "02" | "兼职" => Self::PartTime,
      "03" | "实习" => Self::Intern,
      _ => return None,
    })
  }
}

/// `--kind`: single jobs or hiring announcements (`recruitType`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Kind {
  /// 职位: one job
  #[value(alias = "职位")]
  Job,
  /// 公告: an employer's announcement of several jobs
  #[value(alias = "公告")]
  Announcement,
}

impl Kind {
  pub fn code(self) -> &'static str {
    match self {
      Self::Job => "0",
      Self::Announcement => "1",
    }
  }
}

/// `--degree`: the education asked for, at least (`degreeCode`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Degree {
  /// 不限: no requirement
  #[value(alias = "不限")]
  Any,
  /// 专科及以上
  #[value(alias = "专科")]
  College,
  /// 本科及以上
  #[value(alias = "本科")]
  Bachelor,
  /// 硕士及以上
  #[value(alias = "硕士")]
  Master,
  /// 博士及以上
  #[value(alias = "博士")]
  Phd,
}

impl Degree {
  pub fn code(self) -> &'static str {
    match self {
      Self::Any => "51",
      Self::College => "41",
      Self::Bachelor => "31",
      Self::Master => "11",
      Self::Phd => "01",
    }
  }
}

/// `--salary`: monthly pay (`monthPay`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Salary {
  /// Below 2k a month (2K以下)
  #[value(name = "0-2k")]
  Below2k,
  /// 2k-5k
  #[value(name = "2-5k")]
  From2k,
  /// 5k-10k
  #[value(name = "5-10k")]
  From5k,
  /// 10k-15k
  #[value(name = "10-15k")]
  From10k,
  /// 15k-25k
  #[value(name = "15-25k")]
  From15k,
  /// 25k-50k
  #[value(name = "25-50k")]
  From25k,
  /// 50k and more (50K以上)
  #[value(name = "50k+")]
  Above50k,
  /// To be agreed (面议)
  #[value(alias = "面议")]
  Negotiable,
}

impl Salary {
  pub fn code(self) -> &'static str {
    match self {
      Self::Below2k => "2",
      Self::From2k => "2-5",
      Self::From5k => "5-10",
      Self::From10k => "10-15",
      Self::From15k => "15-25",
      Self::From25k => "25-50",
      Self::Above50k => "50",
      Self::Negotiable => "0",
    }
  }
}

/// `--property`: who owns the company (公司性质, `property`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Property {
  /// 国有企业
  #[value(alias = "国企")]
  State,
  /// 股份制企业
  JointStock,
  /// 民营企业
  #[value(alias = "民企")]
  Private,
  /// 上市公司
  Listed,
  /// 港澳台公司
  Hmt,
  /// 合资企业
  JointVenture,
  /// 外商独资/外企代表处
  #[value(alias = "外企")]
  Foreign,
  /// 机关/事业单位/非营利机构
  #[value(alias = "事业单位")]
  Public,
  /// 其他
  Other,
}

impl Property {
  pub fn code(self) -> &'static str {
    match self {
      Self::State => "国有企业",
      Self::JointStock => "股份制企业",
      Self::Private => "民营企业",
      Self::Listed => "上市公司",
      Self::Hmt => "港澳台公司",
      Self::JointVenture => "合资企业",
      Self::Foreign => "外商独资/外企代表处",
      Self::Public => "机关/事业单位/非营利机构",
      Self::Other => "其他",
    }
  }
}

/// `--source`: where a job was posted (职位来源).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Source {
  /// 国家大学生就业服务平台 itself
  Platform,
  /// The provinces' and schools' own job sites (省市及高校)
  Schools,
  /// BOSS直聘
  Boss,
  /// 中华英才网
  Chinahr,
  /// 猎聘
  Liepin,
  /// 全联人才在线
  Qlrc,
  /// 前程无忧
  #[value(name = "51job")]
  Job51,
}

impl Source {
  /// `sourcesType` and `sourcesName`.
  pub fn codes(self) -> (&'static str, &'static str) {
    match self {
      Self::Platform => ("", "0"),
      Self::Schools => ("1", ""),
      Self::Boss => ("0", "2000846399"),
      Self::Chinahr => ("0", "sx4tfd53g3drg3lhcabun0cjmylq8jan"),
      Self::Liepin => ("0", "2000846397"),
      Self::Qlrc => ("0", "wppuiz6fu2k6pi030gvd45l8wky8l1ex"),
      Self::Job51 => ("0", "19etdf5giehhg58vzrf5m0en8hldvkvw"),
    }
  }
}

/// `search --filter`: a job type, or announcements.
pub const SEARCH_FILTERS: &[&str] = &["full-time", "part-time", "intern", "announcement"];

/// A name without the 省 / 市 / 自治区 … the site may add.
fn short(name: &str) -> &str {
  for suffix in ["特别行政区", "自治区", "自治州", "地区", "省", "市", "盟"] {
    if let Some(rest) = name.strip_suffix(suffix)
      && !rest.is_empty()
    {
      return rest;
    }
  }
  name
}

/// Cities and districts with their codes (`110100` 北京市, `110108` 海淀区).
async fn places(api: &Api) -> Result<Vec<(String, String)>> {
  let store = &api.ctx.store;
  let list = match store.cache_get::<Value>(AREAS, AREAS_TTL) {
    Some(v) => v,
    None => {
      let params = [("type", "areacodejc".to_owned())];
      let v = api
        .json("/student/code", &params, "/student/jobs/index.html")
        .await?;
      if v.as_array().is_some_and(|a| !a.is_empty()) {
        store.cache_put(AREAS, &v);
      }
      v
    }
  };
  Ok(
    list
      .as_array()
      .map(Vec::as_slice)
      .unwrap_or_default()
      .iter()
      .filter_map(|p| Some((p.str("value")?, p.str("name")?)))
      .collect(),
  )
}

fn area(code: &str, name: &str, province: Option<&str>) -> Collection {
  let mut item = Collection {
    id: code.to_owned(),
    kind: if code.len() == 2 { "province" } else { "area" }.into(),
    name: name.to_owned(),
    description: province.map(str::to_owned),
    ..Collection::default()
  };
  item.extra.insert("code".into(), json!(code));
  item
}

/// The province a city or district code is in.
fn province_of(code: &str) -> Option<&'static str> {
  let prefix = code.get(..2)?;
  PROVINCES
    .iter()
    .find(|(c, _)| *c == prefix)
    .map(|(_, name)| *name)
}

/// The provinces, or the places whose name contains `name`.
pub async fn areas(api: &Api, name: Option<&str>) -> Result<Vec<Collection>> {
  let provinces = PROVINCES.iter().map(|(c, n)| area(c, n, None));
  let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) else {
    return Ok(provinces.collect());
  };
  let mut found: Vec<Collection> = provinces.filter(|p| p.name.contains(name)).collect();
  found.extend(
    places(api)
      .await?
      .iter()
      .filter(|(_, n)| n.contains(name))
      .map(|(c, n)| area(c, n, province_of(c))),
  );
  Ok(found)
}

/// The code of `--city`: a code as it is, a province, else a city or
/// district by its name (北京, 杭州, 海淀区), the first that contains it.
pub async fn area_code(api: &Api, arg: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  let wanted = short(arg);
  if let Some((code, _)) = PROVINCES.iter().find(|(_, n)| *n == wanted) {
    return Ok((*code).to_owned());
  }
  let places = places(api).await?;
  places
    .iter()
    .find(|(_, n)| n == arg || short(n) == wanted)
    .or_else(|| places.iter().find(|(_, n)| n.contains(wanted)))
    .map(|(c, _)| c.clone())
    .ok_or_else(|| {
      Error::input(format!(
        "NCSS knows no area {arg} (`atp ncss areas NAME` looks them up)"
      ))
    })
}

/// A code of a fixed table by its code or (a part of) its name.
fn pick(table: &[(&str, &str)], arg: &str, what: &str, listing: &str) -> Result<String> {
  let arg = arg.trim();
  table
    .iter()
    .find(|(c, n)| *c == arg || *n == arg)
    .or_else(|| table.iter().find(|(_, n)| n.contains(arg)))
    .map(|(c, _)| (*c).to_owned())
    .ok_or_else(|| Error::input(format!("NCSS has no {what} {arg} (`atp ncss {listing}`)")))
}

/// `--category`: a job category by its code or name (计算机, 销售类 …).
pub fn category(arg: &str) -> Result<String> {
  pick(CATEGORIES, arg, "job category", "categories")
}

/// `--industry`: an industry by its code or name (计算机软件, 银行 …).
pub fn industry(arg: &str) -> Result<String> {
  pick(INDUSTRIES, arg, "industry", "industries")
}

/// The job categories.
pub fn categories() -> Vec<Collection> {
  CATEGORIES
    .iter()
    .map(|(code, name)| Collection {
      id: (*code).to_owned(),
      kind: "category".into(),
      name: (*name).to_owned(),
      ..Collection::default()
    })
    .collect()
}

/// The industries, each with its group.
pub fn industries() -> Vec<Collection> {
  INDUSTRIES
    .iter()
    .map(|(code, name)| Collection {
      id: (*code).to_owned(),
      kind: "industry".into(),
      name: (*name).to_owned(),
      description: INDUSTRY_GROUPS
        .iter()
        .find(|(group, _)| code.starts_with(group))
        .map(|(_, group)| (*group).to_owned()),
      ..Collection::default()
    })
    .collect()
}
