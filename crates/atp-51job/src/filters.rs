//! The codes of the search's filters, from the dictionaries the site's
//! search page loads (`d_search_providesalary`, `d_search_workyear`,
//! `d_search_degreefrom`, `d_search_cottype`, `d_search_companysize`,
//! `d_search_jobterm`), its sort buttons, and its city picker (`cities.txt`:
//! code, name and Latin name per line; 全国 first, then the hot cities,
//! every city from A to Z, the provinces, Hong Kong, Macao, Taiwan and
//! abroad).

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

/// `--salary`: monthly pay (`salary`, 薪资范围).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Pay {
  /// Below 8,000 yuan (8千以下)
  #[value(name = "0-8k")]
  Below8k,
  /// 8,000 to 10,000 (0.8-1万)
  #[value(name = "8-10k")]
  From8k,
  /// 10,000 to 15,000 (1-1.5万)
  #[value(name = "10-15k")]
  From10k,
  /// 15,000 to 20,000 (1.5-2万)
  #[value(name = "15-20k")]
  From15k,
  /// 20,000 to 30,000 (2-3万)
  #[value(name = "20-30k")]
  From20k,
  /// 30,000 to 40,000 (3-4万)
  #[value(name = "30-40k")]
  From30k,
  /// 40,000 to 50,000 (4-5万)
  #[value(name = "40-50k")]
  From40k,
  /// 50,000 and more (5万以上)
  #[value(name = "50k+")]
  Above50k,
}

codes!(Pay {
  Below8k => "201", From8k => "06", From10k => "07", From15k => "08", From20k => "09",
  From30k => "10", From40k => "11", Above50k => "12",
});

/// `--experience`: years of work asked for (`workYear`, 工作年限).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Experience {
  /// Students and new graduates (在校生/应届生)
  #[value(alias = "graduate")]
  Student,
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
  /// No experience needed (无需经验)
  #[value(name = "none")]
  NotNeeded,
}

codes!(Experience {
  Student => "01", From1 => "02", From3 => "03", From5 => "04", Above10 => "05", NotNeeded => "06",
});

/// `--degree`: the education asked for (`degree`, 学历要求).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Degree {
  /// 初中及以下
  #[value(alias = "初中")]
  MiddleSchool,
  /// 高中/中技/中专
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
  /// No degree asked for (无学历要求)
  #[value(name = "none")]
  NotNeeded,
}

codes!(Degree {
  MiddleSchool => "01", HighSchool => "02", College => "03", Bachelor => "04", Master => "05",
  Phd => "06", NotNeeded => "07",
});

/// `--company-type`: who owns the company (`companyType`, 公司性质).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CompanyType {
  /// 国企
  #[value(alias = "国企")]
  State,
  /// 外资（欧美）
  Western,
  /// 外资（非欧美）
  Foreign,
  /// 已上市
  #[value(alias = "上市")]
  Listed,
  /// 合资
  #[value(alias = "合资")]
  JointVenture,
  /// 民营
  #[value(alias = "民营")]
  Private,
  /// 外企代表处
  Representative,
  /// 政府机关
  Government,
  /// 事业单位
  #[value(alias = "事业单位")]
  Institution,
  /// 非营利组织
  Nonprofit,
  /// 创业公司
  #[value(alias = "创业公司")]
  Startup,
}

codes!(CompanyType {
  State => "04", Western => "01", Foreign => "02", Listed => "10", JointVenture => "03",
  Private => "05", Representative => "06", Government => "07", Institution => "08",
  Nonprofit => "09", Startup => "11",
});

/// `--size`: people at the company (`companySize`, 公司规模).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Size {
  /// Fewer than 50
  #[value(name = "0-50")]
  Below50,
  #[value(name = "50-150")]
  From50,
  #[value(name = "150-500")]
  From150,
  #[value(name = "500-1000")]
  From500,
  #[value(name = "1000-5000")]
  From1000,
  #[value(name = "5000-10000")]
  From5000,
  /// 10,000 and more
  #[value(name = "10000+")]
  Above10000,
}

codes!(Size {
  Below50 => "01", From50 => "02", From150 => "03", From500 => "04", From1000 => "05",
  From5000 => "06", Above10000 => "07",
});

/// `--type` and `search --filter`: the kind of job (`jobType`, 工作类型).
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

codes!(JobType {
  FullTime => "01", PartTime => "02", Intern => "03",
});

impl JobType {
  /// The name `search --filter` takes, and `job_type` of the job keys.
  pub fn name(self) -> &'static str {
    match self {
      Self::FullTime => "full-time",
      Self::PartTime => "part-time",
      Self::Intern => "intern",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::FullTime, Self::PartTime, Self::Intern]
      .into_iter()
      .find(|t| t.name() == name)
  }

  /// The kind of a job by the site's word for it (`termStr`).
  pub fn from_word(word: &str) -> Option<Self> {
    Some(match word {
      "全职" => Self::FullTime,
      "兼职" => Self::PartTime,
      "实习" => Self::Intern,
      _ => return None,
    })
  }
}

/// `search --filter`.
pub const SEARCH_FILTERS: &[&str] = &["full-time", "part-time", "intern"];

/// `--sort`: the order of the search page's buttons (`sortType`): 综合排序,
/// 最新优先, 薪资优先, 活跃职位优先.
pub const SORTS: &[&str] = &["relevance", "newest", "salary", "active"];

/// The `sortType` of a `--sort` value.
pub fn sort(name: Option<&str>) -> &'static str {
  match name {
    Some("newest") => "1",
    Some("salary") => "3",
    Some("active") => "5",
    _ => "0",
  }
}

/// Code, name and Latin name of every place.
const CITIES: &str = include_str!("cities.txt");
/// The code of all of China.
pub const NATIONWIDE: &str = "000000";
/// How many of the first places are 全国 and the hot cities.
const HOT: usize = 26;

/// Every place as `(code, name, Latin name)`.
fn cities() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
  CITIES.lines().filter_map(|line| {
    let mut parts = line.split(' ');
    Some((parts.next()?, parts.next()?, parts.next().unwrap_or("")))
  })
}

/// The hot cities, or the places whose name contains `name`.
pub fn list(name: Option<&str>) -> Vec<Collection> {
  let name = name.map(str::trim).filter(|n| !n.is_empty());
  let picked: Vec<_> = match name {
    Some(name) => {
      let lower = name.to_lowercase();
      cities()
        .filter(|(_, n, latin)| n.contains(name) || latin.to_lowercase().contains(&lower))
        .collect()
    }
    None => cities().take(HOT).collect(),
  };
  picked
    .into_iter()
    .map(|(code, name, latin)| {
      let mut city = Collection {
        id: code.to_owned(),
        kind: "city".into(),
        name: name.to_owned(),
        ..Collection::default()
      };
      city.extra.insert("code".into(), json!(code));
      if !latin.is_empty() {
        city.extra.insert("latin".into(), json!(latin));
      }
      city
    })
    .collect()
}

/// The code of `--city`: a code as it is, or a place's name (北京, 杭州,
/// 广东省; 市 and 省 may be left out) or Latin name (`hangzhou`).
pub fn city(arg: &str) -> Result<String> {
  let arg = arg.trim();
  if arg.len() == 6 && arg.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(arg.to_owned());
  }
  let short = arg.trim_end_matches(['市', '省']);
  if let Some((code, _, _)) = cities().find(|(_, name, _)| name.trim_end_matches('省') == short) {
    return Ok(code.to_owned());
  }
  let latin: Vec<_> = cities()
    .filter(|(_, _, latin)| latin.eq_ignore_ascii_case(arg))
    .collect();
  match latin.as_slice() {
    [(code, _, _)] => Ok((*code).to_owned()),
    [] => Err(Error::input(format!(
      "51job knows no city {arg} (`atp 51job cities NAME` looks them up)"
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
