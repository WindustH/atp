//! The values of the search's filters, as the search page offers them:
//! internships or campus jobs, days a week, months, degree, daily pay,
//! return offers and how recently jobs were published.

/// `search --filter` and `jobs --kind`: internships (实习) or campus jobs
/// (校招).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
  /// 实习: internships, paid by the day
  #[default]
  #[value(alias = "实习")]
  Intern,
  /// 校招: campus hiring of new graduates, paid by the month
  #[value(alias = "校招")]
  Campus,
}

impl Kind {
  pub fn name(self) -> &'static str {
    match self {
      Self::Intern => "intern",
      Self::Campus => "campus",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::Intern, Self::Campus]
      .into_iter()
      .find(|kind| kind.name() == name)
  }
}

/// `search --filter`.
pub const SEARCH_FILTERS: &[&str] = &["intern", "campus"];
/// `search --sort`: the site's order or the newest first.
pub const SORTS: &[&str] = &["default", "new"];

/// `--months`: how long the internship lasts (实习月份).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Months {
  /// One month
  #[value(name = "1")]
  One,
  /// Two months
  #[value(name = "2")]
  Two,
  /// Three months
  #[value(name = "3")]
  Three,
  /// More than three months
  #[value(name = "3+")]
  More,
}

impl Months {
  pub fn code(self) -> &'static str {
    match self {
      Self::One => "1",
      Self::Two => "2",
      Self::Three => "3",
      Self::More => "4",
    }
  }
}

/// `--degree`: the education asked for (学历要求).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Degree {
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

impl Degree {
  pub fn code(self) -> &'static str {
    match self {
      Self::College => "大专",
      Self::Bachelor => "本科",
      Self::Master => "硕士",
      Self::Phd => "博士",
    }
  }
}

/// `--pay`: daily pay in yuan (日薪).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Pay {
  /// Below 100 a day
  #[value(name = "0-100")]
  Below100,
  /// 100-150 a day
  #[value(name = "100-150")]
  From100,
  /// 150-200 a day
  #[value(name = "150-200")]
  From150,
  /// 200-300 a day
  #[value(name = "200-300")]
  From200,
  /// 300 and more a day
  #[value(name = "300+")]
  Above300,
}

impl Pay {
  pub fn code(self) -> &'static str {
    match self {
      Self::Below100 => "0-100",
      Self::From100 => "100-150",
      Self::From150 => "150-200",
      Self::From200 => "200-300",
      Self::Above300 => "300-",
    }
  }
}

/// `--offer`: whether the internship may turn into a job (转正机会).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Offer {
  /// 提供转正
  Yes,
  /// 不提供转正
  No,
  /// 面议: to be agreed
  Maybe,
}

impl Offer {
  pub fn code(self) -> &'static str {
    match self {
      Self::Yes => "entry",
      Self::No => "noentry",
      Self::Maybe => "notsure",
    }
  }
}

/// `--published`: how recently the job was published (发布时间).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Published {
  /// Within a day
  Day,
  /// Within a week
  Week,
  /// Within a month
  Month,
}

impl Published {
  pub fn code(self) -> &'static str {
    match self {
      Self::Day => "day",
      Self::Week => "wek",
      Self::Month => "mon",
    }
  }
}

/// `apply --start`: when you can start (到岗时间).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Start {
  /// Within a week
  #[value(name = "1w")]
  Week,
  /// Within two weeks
  #[value(name = "2w")]
  TwoWeeks,
  /// Within a month
  #[value(name = "1m")]
  Month,
  /// Within three months
  #[value(name = "3m")]
  ThreeMonths,
}

impl Start {
  pub fn label(self) -> &'static str {
    match self {
      Self::Week => "1周内",
      Self::TwoWeeks => "2周内",
      Self::Month => "1个月内",
      Self::ThreeMonths => "3个月内",
    }
  }
}

/// `apply --duration`: how long you can stay (实习时长).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Stay {
  /// Up to three months
  #[value(name = "0-3m")]
  Short,
  /// Three to six months
  #[value(name = "3-6m")]
  Medium,
  /// More than six months
  #[value(name = "6m+")]
  Long,
}

impl Stay {
  pub fn label(self) -> &'static str {
    match self {
      Self::Short => "3个月以内",
      Self::Medium => "3-6个月",
      Self::Long => "6个月以上",
    }
  }
}

/// `applied --stage`: the tabs of the site's list of applications.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Stage {
  /// 被查看: read by the company
  Viewed,
  /// 初筛: in the first screening
  Screening,
  /// 笔试: invited to a written test
  Test,
  /// 面试: invited to an interview
  Interview,
  /// 不合适: turned down
  Rejected,
  /// 录用: hired
  Hired,
}

impl Stage {
  pub fn code(self) -> &'static str {
    match self {
      Self::Viewed => "checked",
      Self::Screening => "undeter",
      Self::Test => "written",
      Self::Interview => "notify",
      Self::Rejected => "reject",
      Self::Hired => "employed",
    }
  }
}
