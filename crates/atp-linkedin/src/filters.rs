//! What LinkedIn's job search takes from a visitor without an account:
//! words (`keywords`), a place (`location` as words, or `geoId`), how
//! recently jobs were posted (`f_TPR`) and companies (`f_C`).
//!
//! Its jobs pages also offer workplace (`f_WT`), experience level (`f_E`),
//! job type (`f_JT`) and the order (`sortBy`), but the search a visitor's
//! pages load ignores them (internships asked for, staff engineers
//! answered), so ATP does not offer them rather than pretend to apply
//! them. A job's page tells its employment type and seniority level.

/// `--posted`: how recently the job was posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Posted {
  /// In the past 24 hours
  #[value(alias = "24h")]
  Day,
  /// In the past week
  Week,
  /// In the past month
  Month,
}

impl Posted {
  fn code(self) -> &'static str {
    match self {
      Self::Day => "r86400",
      Self::Week => "r604800",
      Self::Month => "r2592000",
    }
  }
}

/// What a search is narrowed to.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  pub words: String,
  /// A place in words (`Berlin`, `United States`).
  pub location: Option<String>,
  /// A place by LinkedIn's number for it (`geoId`).
  pub geo: Option<String>,
  pub posted: Option<Posted>,
  /// Company numbers (`f_C`), each a number or several joined by commas.
  pub companies: Vec<String>,
}

impl Filter {
  /// `search WORDS`.
  pub fn words(words: &str) -> Self {
    Self {
      words: words.trim().to_owned(),
      ..Self::default()
    }
  }

  /// The query of the search, without the offset.
  pub fn query(&self) -> Vec<(&'static str, String)> {
    let trimmed = |s: &Option<String>| s.as_deref().unwrap_or_default().trim().to_owned();
    [
      ("keywords", self.words.trim().to_owned()),
      ("location", trimmed(&self.location)),
      ("geoId", trimmed(&self.geo)),
      (
        "f_TPR",
        self.posted.map(Posted::code).unwrap_or_default().into(),
      ),
      ("f_C", self.companies.join(",")),
    ]
    .into_iter()
    .filter(|(_, value)| !value.is_empty())
    .collect()
  }
}
