//! The codes of the search's filters, as the site's own data gives them:
//! pay, experience, degree, company size and funding stage and job type
//! from `/wapi/zpgeek/search/job/condition.json`, industries from
//! `/wapi/zpCommon/data/industry.json` (`industries.txt`: code, name and
//! the group per line) and cities from `/wapi/zpCommon/data/city.json`
//! (`cities.txt`: code, name and pinyin per line; 全国 and the hot cities
//! first, then every city province by province).

use atp_core::{Collection, Error, Result, json};

/// The codes of the options below, in the site's words.
pub trait Code {
  fn code(self) -> &'static str;
}

macro_rules! codes {
  ($name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
    impl Code for $name {
      fn code(self) -> &'static str {
        match self {
          $(Self::$variant => $code),+
        }
      }
    }
  };
}

/// `--salary`: monthly pay (`salary`, 薪资待遇).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Pay {
  /// Below 3,000 yuan (3K以下)
  #[value(name = "0-3k")]
  Below3k,
  /// 3,000 to 5,000
  #[value(name = "3-5k")]
  From3k,
  /// 5,000 to 10,000
  #[value(name = "5-10k")]
  From5k,
  /// 10,000 to 20,000
  #[value(name = "10-20k")]
  From10k,
  /// 20,000 to 50,000
  #[value(name = "20-50k")]
  From20k,
  /// 50,000 and more (50K以上)
  #[value(name = "50k+")]
  Above50k,
}

codes!(Pay {
  Below3k => "402", From3k => "403", From5k => "404", From10k => "405", From20k => "406",
  Above50k => "407",
});

/// `--experience`: the experience asked for (`experience`, 工作经验).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Experience {
  /// Students (在校生)
  #[value(alias = "在校生")]
  Student,
  /// New graduates (应届生)
  #[value(alias = "应届生")]
  Graduate,
  /// Any experience (经验不限)
  #[value(name = "none", alias = "经验不限")]
  NotNeeded,
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

codes!(Experience {
  Student => "108", Graduate => "102", NotNeeded => "101", UpTo1 => "103", From1 => "104",
  From3 => "105", From5 => "106", Above10 => "107",
});

/// `--degree`: the education asked for (`degree`, 学历要求).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Degree {
  /// 初中及以下
  #[value(alias = "初中")]
  MiddleSchool,
  /// 中专/中技
  #[value(alias = "中专")]
  Technical,
  /// 高中
  #[value(alias = "高中")]
  HighSchool,
  /// 大专
  #[value(alias = "大专")]
  College,
  /// 本科
  #[value(alias = "本科")]
  Bachelor,
  /// 硕士
  #[value(alias = "硕士")]
  Master,
  /// 博士
  #[value(alias = "博士")]
  Phd,
}

codes!(Degree {
  MiddleSchool => "209", Technical => "208", HighSchool => "206", College => "202",
  Bachelor => "203", Master => "204", Phd => "205",
});

/// `--size`: people at the company (`scale`, 公司规模).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Size {
  /// Fewer than 20
  #[value(name = "0-20")]
  Below20,
  #[value(name = "20-99")]
  From20,
  #[value(name = "100-499")]
  From100,
  #[value(name = "500-999")]
  From500,
  #[value(name = "1000-9999")]
  From1000,
  /// 10,000 and more
  #[value(name = "10000+")]
  Above10000,
}

codes!(Size {
  Below20 => "301", From20 => "302", From100 => "303", From500 => "304", From1000 => "305",
  Above10000 => "306",
});

/// `--stage`: the company's funding (`stage`, 融资阶段).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Stage {
  /// 未融资
  Unfunded,
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
  /// 不需要融资
  #[value(name = "self-funded")]
  SelfFunded,
}

codes!(Stage {
  Unfunded => "801", Angel => "802", A => "803", B => "804", C => "805", DPlus => "806",
  Listed => "807", SelfFunded => "808",
});

/// `--type` and `search --filter`: the kind of job (`jobType`, 求职类型).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum JobType {
  /// 全职
  #[value(alias = "全职")]
  FullTime,
  /// 兼职
  #[value(alias = "兼职")]
  PartTime,
}

codes!(JobType {
  FullTime => "1901", PartTime => "1903",
});

impl JobType {
  /// The name `search --filter` takes, and `job_type` of the job keys.
  pub fn name(self) -> &'static str {
    match self {
      Self::FullTime => "full-time",
      Self::PartTime => "part-time",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::FullTime, Self::PartTime]
      .into_iter()
      .find(|t| t.name() == name)
  }
}

/// `search --filter`.
pub const SEARCH_FILTERS: &[&str] = &["full-time", "part-time"];

/// Code, name and pinyin of every city.
const CITIES: &str = include_str!("cities.txt");
/// The code of all of China.
pub const NATIONWIDE: &str = "100010000";
/// How many of the first cities are 全国 and the hot cities.
const HOT: usize = 15;

/// Every city as `(code, name, pinyin)`.
fn cities() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
  CITIES.lines().filter_map(|line| {
    let mut parts = line.split(' ');
    Some((parts.next()?, parts.next()?, parts.next().unwrap_or("")))
  })
}

/// A row of a code table, as a collection.
fn row(kind: &str, code: &str, name: &str, extra: (&str, &str)) -> Collection {
  let mut item = Collection {
    id: code.to_owned(),
    kind: kind.into(),
    name: name.to_owned(),
    ..Collection::default()
  };
  item.extra.insert("code".into(), json!(code));
  if !extra.1.is_empty() {
    item.extra.insert(extra.0.into(), json!(extra.1));
  }
  item
}

/// The hot cities, or the cities whose name or pinyin contains `name`.
pub fn city_list(name: Option<&str>) -> Vec<Collection> {
  let name = name.map(str::trim).filter(|n| !n.is_empty());
  let picked: Vec<_> = match name {
    Some(name) => {
      let lower = name.to_lowercase();
      cities()
        .filter(|(_, n, pinyin)| n.contains(name) || pinyin.contains(&lower))
        .collect()
    }
    None => cities().take(HOT).collect(),
  };
  picked
    .into_iter()
    .map(|(code, name, pinyin)| row("city", code, name, ("pinyin", pinyin)))
    .collect()
}

/// The code of `--city`: a code as it is, or a city's name (北京, 杭州;
/// 市 may be left out) or pinyin (`hangzhou`).
pub fn city(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if arg.len() == 9 && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  let short = arg.trim_end_matches('市');
  if let Some((code, _, _)) = cities().find(|(_, name, _)| *name == short) {
    return Ok(code.to_owned());
  }
  let pinyin: Vec<_> = cities()
    .filter(|(_, _, pinyin)| pinyin.eq_ignore_ascii_case(arg))
    .collect();
  match pinyin.as_slice() {
    [(code, _, _)] => Ok((*code).to_owned()),
    [] => Err(Error::input(format!(
      "BOSS Zhipin knows no city {arg} (`atp boss cities NAME` looks them up)"
    ))),
    several => {
      let names: Vec<String> = several
        .iter()
        .map(|(c, n, _)| format!("{n} ({c})"))
        .collect();
      Err(Error::input(format!(
        "{arg} may be {}; give the Chinese name or the code",
        names.join(" or ")
      )))
    }
  }
}

/// Code, name and group of every industry.
const INDUSTRIES: &str = include_str!("industries.txt");

/// Every industry as `(code, name, group)`.
fn industries() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
  INDUSTRIES.lines().filter_map(|line| {
    let mut parts = line.split(' ');
    Some((parts.next()?, parts.next()?, parts.next().unwrap_or("")))
  })
}

pub fn industry_list() -> Vec<Collection> {
  industries()
    .map(|(code, name, group)| row("industry", code, name, ("group", group)))
    .collect()
}

/// The code of `--industry`: a code as it is, or an industry's name
/// (人工智能, 游戏, 计算机软件).
pub fn industry(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if arg.len() == 6 && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  industries()
    .find(|(_, name, _)| *name == arg)
    .or_else(|| industries().find(|(_, name, _)| name.contains(arg)))
    .map(|(code, _, _)| code.to_owned())
    .ok_or_else(|| {
      Error::input(format!(
        "BOSS Zhipin knows no industry {arg} (`atp boss industries` lists them)"
      ))
    })
}
