//! Trains and days as arguments.
//!
//! - trains: the code (`G102`, `K1`, `1461`), or a train of a `trains`
//!   list as its `id` names it: `G102/AOH-VNP/2026-10-20` (the code, the
//!   telecodes of the stations queried, the day). Parts after the code
//!   may come alone or in any order;
//! - days: `YYYY-MM-DD`, today in Beijing by default.

use atp_core::{Error, Result};
use jiff::civil::Date;
use jiff::tz::{Offset, TimeZone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainRef {
  /// `G102`.
  pub code: String,
  /// Telecodes of the stations a list was queried between.
  pub from: Option<String>,
  pub to: Option<String>,
  pub date: Option<Date>,
}

fn telecode(s: &str) -> bool {
  (2..=4).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase())
}

/// A train code: an optional letter and up to five digits.
fn code(s: &str) -> bool {
  let digits = s.trim_start_matches(|c: char| c.is_ascii_alphabetic());
  s.len() - digits.len() <= 1
    && (1..=5).contains(&digits.len())
    && digits.bytes().all(|b| b.is_ascii_digit())
}

pub fn train(arg: &str) -> Result<TrainRef> {
  let invalid = || {
    Error::input(format!(
      "not a train: {arg} (expected a code such as G102, or an id of a `trains` list)"
    ))
  };
  let mut parts = arg.trim().split('/');
  let first = parts.next().unwrap_or_default().to_uppercase();
  if !code(&first) {
    return Err(invalid());
  }
  let mut train = TrainRef {
    code: first,
    from: None,
    to: None,
    date: None,
  };
  for part in parts {
    if let Some((from, to)) = part
      .split_once('-')
      .filter(|(a, b)| telecode(a) && telecode(b))
    {
      train.from = Some(from.to_owned());
      train.to = Some(to.to_owned());
    } else if let Ok(date) = part.parse::<Date>() {
      train.date = Some(date);
    } else {
      return Err(invalid());
    }
  }
  Ok(train)
}

/// The id of a train of a list: `G102/AOH-VNP/2026-10-20`.
pub fn train_id(code: &str, from: &str, to: &str, date: Date) -> String {
  format!("{code}/{from}-{to}/{date}")
}

pub fn day(text: &str) -> Result<Date> {
  text
    .trim()
    .parse()
    .map_err(|_| Error::input(format!("not a day: {text} (use YYYY-MM-DD)")))
}

/// Today in Beijing.
pub fn today() -> Date {
  let beijing = TimeZone::fixed(Offset::constant(8));
  jiff::Timestamp::now().to_zoned(beijing).date()
}
