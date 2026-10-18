//! The codes of the search's filters: cities (the mobile site's city list,
//! `cities.txt`: code and name per line, 全国 first, then the hot cities),
//! experience, education and job type as option values, monthly pay as
//! a range. The codes were checked against the answers of the search:
//! each filter keeps the jobs the site labels with it.

use atp_core::{Collection, Error, Result, json};

/// Code and name of every city, 全国 (489) first.
const CITIES: &str = include_str!("cities.txt");
/// The code of all of China.
pub const NATIONWIDE: &str = "489";
/// How many of the first cities are the hot ones the site offers first.
const HOT: usize = 29;

/// `--experience`: years of work asked for (`S_SOU_WORK_EXPERIENCE`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Experience {
  /// Less than a year (1年以下)
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
      Self::UpTo1 => "0001",
      Self::From1 => "0103",
      Self::From3 => "0305",
      Self::From5 => "0510",
      Self::Above10 => "1099",
    }
  }
}

/// `--degree`: the education asked for (`S_SOU_EDUCATION_LOWESTLEVEL`).
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
  /// 高中
  #[value(alias = "高中")]
  HighSchool,
}

impl Degree {
  pub fn code(self) -> &'static str {
    match self {
      Self::Phd => "1",
      Self::Master => "3",
      Self::Bachelor => "4",
      Self::College => "5",
      Self::HighSchool => "7",
    }
  }
}

/// `--type` and `search --filter`: the kind of job (`S_SOU_POSITION_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum JobType {
  /// 全职
  #[value(alias = "全职")]
  FullTime,
  /// 兼职/临时
  #[value(alias = "兼职")]
  PartTime,
  /// 实习
  #[value(alias = "实习")]
  Intern,
  /// 校园: campus hiring of new graduates
  #[value(alias = "校园")]
  Campus,
}

impl JobType {
  pub fn code(self) -> &'static str {
    match self {
      Self::FullTime => "2",
      Self::PartTime => "1",
      Self::Intern => "4",
      Self::Campus => "5",
    }
  }

  /// The name `search --filter` takes.
  pub fn name(self) -> &'static str {
    match self {
      Self::FullTime => "full-time",
      Self::PartTime => "part-time",
      Self::Intern => "intern",
      Self::Campus => "campus",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::FullTime, Self::PartTime, Self::Intern, Self::Campus]
      .into_iter()
      .find(|t| t.name() == name)
  }

  /// `job_type` of the shared job keys, by the card's `workTypeCode`.
  pub fn job_key(code: &str) -> Option<&'static str> {
    Some(match code {
      "2" | "5" => "full-time",
      "1" => "part-time",
      "4" => "intern",
      _ => return None,
    })
  }
}

/// `search --filter`.
pub const SEARCH_FILTERS: &[&str] = &["full-time", "part-time", "intern", "campus"];

/// Every city as `(code, name)`.
fn cities() -> impl Iterator<Item = (&'static str, &'static str)> {
  CITIES.lines().filter_map(|line| line.split_once(' '))
}

/// The hot cities, or those whose name contains `name`.
pub fn list(name: Option<&str>) -> Vec<Collection> {
  let name = name.map(str::trim).filter(|n| !n.is_empty());
  let picked: Vec<(&str, &str)> = match name {
    Some(name) => cities().filter(|(_, n)| n.contains(name)).collect(),
    None => cities().take(HOT).collect(),
  };
  picked
    .into_iter()
    .map(|(code, name)| {
      let mut city = Collection {
        id: code.to_owned(),
        kind: "city".into(),
        name: name.to_owned(),
        ..Collection::default()
      };
      city.extra.insert("code".into(), json!(code));
      city
    })
    .collect()
}

/// The code of `--city`: a code as it is, or a city's name (北京, 杭州,
/// 阿拉尔市; 市 may be left out).
pub fn city(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  let short = arg.strip_suffix('市').unwrap_or(arg);
  cities()
    .find(|(_, n)| *n == arg || n.strip_suffix('市').unwrap_or(n) == short)
    .map(|(code, _)| code.to_owned())
    .ok_or_else(|| {
      Error::input(format!(
        "Zhilian knows no city {arg} (`atp zhilian cities NAME` looks them up)"
      ))
    })
}

/// `--salary 15-25` (thousands a month; `15k-25k` too) as the search's
/// `min,max` in yuan.
pub fn salary(arg: &str) -> Result<String> {
  let thousands = |text: &str| {
    text
      .trim()
      .trim_end_matches(['k', 'K'])
      .parse::<f64>()
      .ok()
      .filter(|n| *n >= 0.0)
  };
  let parsed = arg
    .split_once('-')
    .and_then(|(min, max)| Some((thousands(min)?, thousands(max)?)))
    .filter(|(min, max)| min <= max);
  let Some((min, max)) = parsed else {
    return Err(Error::input(format!(
      "--salary takes a monthly range in thousands, as 15-25 or 15k-25k (not {arg})"
    )));
  };
  Ok(format!("{:.0},{:.0}", min * 1000.0, max * 1000.0))
}
