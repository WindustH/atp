//! What job platforms share: the pay of a job ([`Salary`]), read from the
//! pay a platform shows (`15-25K·14薪`, `200-300元/天`, `20-40万/年`,
//! `$100k-150k/yr`, `$117,000.00/yr - $234,000.00/yr`, `184,000 USD -
//! 287,500 USD`, `面议`) or found in a job's title, and written into the
//! job keys of `Post.extra` (see [`Post::extra`](crate::model::Post)).
//!
//! Figures are kept per the period they are paid for (`salary_period`):
//! pay per hour, day or year cannot honestly be turned into pay per month.

use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde_json::{Value, json};

use crate::model::Extra;

/// Currency signs written before an amount, longest first (`CA$` before
/// `$`). `¥` is the yuan, as Chinese sites write it.
const SIGNS: &str = r"US\$|\$US|HK\$|CA\$|AU\$|A\$|NZ\$|MX\$|R\$|CN¥|\$|¥|€|£|₹|₩";

/// ISO codes amounts may be given in, before or after them (`USD 120,000`,
/// `184,000 USD`); `RMB` is the yuan.
const CODES: &[&str] = &[
  "USD", "EUR", "GBP", "JPY", "CAD", "AUD", "NZD", "HKD", "SGD", "INR", "CHF", "SEK", "NOK", "DKK",
  "PLN", "BRL", "MXN", "KRW", "TWD", "CNY", "RMB",
];

/// An amount: `15`, `1.5`, `3,000`, `117,000.00`.
const NUMBER: &str = r"(\d{1,3}(?:,\d{3})+(?:\.\d+)?|\d+(?:\.\d+)?)";

/// The periods pay is given per (`/月`, `/yr`, `per hour`).
const PERIODS: &str = "小时|时|天|日|周|月|年|hour|hr|h|day|week|wk|month|mo|year|yr|annum";

/// An amount (`15`, `1.5`, `3,000`), a range of two (`15-25K`, `1-1.5万`,
/// `$100k-150k`, `$50 to $60`), its currency in front and `以上` / `起` /
/// `+` behind. The first amount of a range may carry its own period or
/// currency code (`$117,000.00/yr - $234,000.00/yr`, `184,000 USD -
/// 287,500 USD`); what follows the range is read by [`AFTER`].
static AMOUNT: LazyLock<Regex> = LazyLock::new(|| {
  let codes = CODES.join("|");
  Regex::new(&format!(
    r"(?i)({SIGNS}|{codes})?\s*{NUMBER}\s*([kw万千])?(?:(?:\s*/\s*(?:{PERIODS})\b|\s*({codes})\b)?(?:\s*(?:-|~|到|至)\s*|\s*\bto\s+)(?:{SIGNS}|{codes})?\s*{NUMBER}\s*([kw万千])?)?(\+|以上|起)?"
  ))
  .expect("valid pattern")
});

/// What may follow an amount: a currency code or word, a word for pay, the
/// period (`/月`, `元/天`, `/yr`, `per hour`, `annually`), `以上` and
/// `·14薪`.
static AFTER: LazyLock<Regex> = LazyLock::new(|| {
  let codes = CODES.join("|");
  Regex::new(&format!(
    r"(?i)^\s*({codes}|美元|美金|元)?\s*(年薪|月薪|底薪|薪资|工资)?\s*(?:(?:/|每|一|per\s*|an?\s+)\s*({PERIODS})|(annually|annual|yearly|monthly|weekly|daily|hourly))?\s*(以上|起)?\s*(?:[·•・*x×]?\s*(\d{{2}})\s*薪)?"
  ))
  .expect("valid pattern")
});

/// Words for pay right before an amount (年薪, 月薪 … may also follow it).
const KEYWORDS: &[&str] = &[
  "年薪", "月薪", "底薪", "薪资", "薪酬", "工资", "待遇", "时薪", "日薪", "月入", "最高",
];

/// What a salary's figures are paid per (`salary_period`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Period {
  Hour,
  Day,
  #[default]
  Month,
  Year,
}

impl Period {
  /// `hour`, `day`, `month` or `year`, as in `salary_period`.
  pub fn name(self) -> &'static str {
    match self {
      Self::Hour => "hour",
      Self::Day => "day",
      Self::Month => "month",
      Self::Year => "year",
    }
  }
}

/// The pay of a job: the text as shown and, where it names them, the
/// figures per [`Period`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Salary {
  /// The pay as shown (`salary`); empty when only the figures are known.
  pub text: String,
  pub min: Option<f64>,
  pub max: Option<f64>,
  /// ISO code (`salary_currency`).
  pub currency: String,
  pub period: Period,
  /// Months paid a year (`salary_months`: 14 for `·14薪`).
  pub months: Option<u32>,
}

impl Salary {
  /// Figures known without a text (a schema.org `baseSalary` …), in CNY.
  pub fn new(min: Option<f64>, max: Option<f64>, period: Period) -> Self {
    Self {
      min,
      max,
      currency: "CNY".into(),
      period,
      ..Self::default()
    }
  }

  /// The pay a platform shows in its own field (`15-25K·14薪`, `1-1.5万`,
  /// `月薪1万-2万`, `8000-12000元/月`, `150/天`, `$25.00/hr - $35.00/hr`,
  /// `面议`): the whole text is the pay. Figures count with a unit, a
  /// currency, a word for pay or a period; a bare number keeps only the
  /// text. Without a period the pay is monthly. `None` for an empty text.
  pub fn parse(text: &str) -> Option<Self> {
    let shown = text.trim();
    if shown.is_empty() {
      return None;
    }
    let text = normalize(shown);
    let found = AMOUNT
      .captures_iter(&text)
      .find_map(|caps| Found::read(&text, &caps, false));
    Some(match found {
      Some(found) => found.salary(shown, false),
      None => Self {
        text: shown.to_owned(),
        ..Self::default()
      },
    })
  }

  /// The pay a title or other free text names, read conservatively: a
  /// range counts only with a unit (K, W, 万, 千) or a currency, a single
  /// amount only with a currency or a word for pay next to it (年薪, 月薪,
  /// 最高 …), and below 100 only when paid by the hour or day (`$50/hr`).
  /// Without a period, amounts from 100,000 up are taken as yearly pay
  /// (unless `·N薪` names the months paid), others as monthly.
  pub fn in_text(text: &str) -> Option<Self> {
    let text = normalize(text);
    let found = AMOUNT
      .captures_iter(&text)
      .find_map(|caps| Found::read(&text, &caps, true))?;
    let shown = text[found.start..found.end].trim().to_owned();
    Some(found.salary(&shown, true))
  }

  /// The job keys: `salary`, `salary_min` / `salary_max` (per
  /// `salary_period`), `salary_currency`, `salary_period`, `salary_months`.
  pub fn fill(&self, extra: &mut Extra) {
    if !self.text.is_empty() {
      extra.insert("salary".into(), json!(self.text));
    }
    if let Some(months) = self.months {
      extra.insert("salary_months".into(), json!(months));
    }
    if self.min.is_none() && self.max.is_none() {
      return;
    }
    if let Some(min) = self.min {
      extra.insert("salary_min".into(), amount(min));
    }
    if let Some(max) = self.max {
      extra.insert("salary_max".into(), amount(max));
    }
    extra.insert("salary_currency".into(), json!(self.currency));
    extra.insert("salary_period".into(), json!(self.period.name()));
  }
}

/// A whole amount as an integer, else with at most two decimals.
fn amount(n: f64) -> Value {
  if (n - n.round()).abs() < 1e-6 && n >= 0.0 {
    json!(n.round() as u64)
  } else {
    json!((n * 100.0).round() / 100.0)
  }
}

/// The periods text may name, weekly pay included (which keeps no figures).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Per {
  Paid(Period),
  Week,
}

/// One amount of a text that reads as pay.
struct Found {
  start: usize,
  end: usize,
  min: Option<f64>,
  max: Option<f64>,
  currency: &'static str,
  period: Option<Per>,
  months: Option<u32>,
}

impl Found {
  /// The pay one amount stands for, if the words around it say it is pay
  /// (`strict` for free text, see [`Salary::in_text`]).
  fn read(text: &str, caps: &Captures, strict: bool) -> Option<Self> {
    let whole = caps.get(0)?;
    let before = text[..whole.start()].trim_end();
    let keyword = KEYWORDS.iter().find(|k| before.ends_with(*k)).copied();
    let after = AFTER.captures(&text[whole.end()..])?;
    // The sign in front, the code inside a range or the code after it.
    let symbol = caps
      .get(1)
      .or(caps.get(4))
      .or(after.get(1))
      .map(|m| m.as_str());
    let named = keyword.or(after.get(2).map(|m| m.as_str()));
    let ranged = caps.get(5).is_some();
    let (unit_a, unit_b) = (unit(caps, 3), unit(caps, 6));
    let has_unit = unit_a.is_some() || unit_b.is_some();
    let per = after.get(3).or(after.get(4)).map(|m| m.as_str());
    let period = period(named, per);
    let pay_words = symbol.is_some() || named.is_some();
    let counts = if strict {
      pay_words || (ranged && has_unit)
    } else {
      pay_words || has_unit || period.is_some()
    };
    if !counts {
      return None;
    }
    let a = number(caps, 2)? * unit_a.or(unit_b).unwrap_or(1.0);
    let hourly_or_daily = matches!(period, Some(Per::Paid(Period::Hour | Period::Day)));
    if strict && !ranged && a < 100.0 && !hourly_or_daily {
      return None;
    }
    let b = number(caps, 5).map(|b| b * unit_b.or(unit_a).unwrap_or(1.0));
    let open = caps.get(7).is_some() || after.get(5).is_some();
    let (min, max) = match b {
      Some(b) => (Some(a), Some(b)),
      None if keyword == Some("最高") => (None, Some(a)),
      None if open => (Some(a), None),
      None => (Some(a), Some(a)),
    };
    let start = keyword.map_or(whole.start(), |k| before.len() - k.len());
    Some(Self {
      start,
      end: whole.end() + after.get(0).map_or(0, |m| m.end()),
      min,
      max,
      currency: currency(symbol.unwrap_or("¥")),
      period,
      months: after.get(6).and_then(|m| m.as_str().parse().ok()),
    })
  }

  /// The salary, with `text` as shown. Without a period the pay is
  /// monthly, or with `guess_year` yearly from 100,000 up when no `·N薪`
  /// says it is monthly; weekly pay keeps no figures.
  fn salary(self, text: &str, guess_year: bool) -> Salary {
    let largest = self.max.or(self.min).unwrap_or_default();
    let yearly = guess_year && self.months.is_none() && largest >= 100_000.0;
    let period = match self.period {
      Some(Per::Paid(period)) => Some(period),
      Some(Per::Week) => None,
      None if yearly => Some(Period::Year),
      None => Some(Period::Month),
    };
    let (min, max) = match period {
      Some(_) => (self.min, self.max),
      None => (None, None),
    };
    Salary {
      text: text.to_owned(),
      min,
      max,
      currency: self.currency.into(),
      period: period.unwrap_or_default(),
      months: self.months,
    }
  }
}

/// The period a word for pay (年薪, 时薪 …) or a period word names.
fn period(keyword: Option<&str>, per: Option<&str>) -> Option<Per> {
  let paid = |period| Some(Per::Paid(period));
  match keyword {
    Some("年薪") => return paid(Period::Year),
    Some("月薪" | "月入" | "底薪") => return paid(Period::Month),
    Some("时薪") => return paid(Period::Hour),
    Some("日薪") => return paid(Period::Day),
    _ => {}
  }
  match per?.to_lowercase().as_str() {
    "小时" | "时" | "hour" | "hr" | "h" | "hourly" => paid(Period::Hour),
    "天" | "日" | "day" | "daily" => paid(Period::Day),
    "月" | "month" | "mo" | "monthly" => paid(Period::Month),
    "年" | "year" | "yr" | "annum" | "annual" | "annually" | "yearly" => paid(Period::Year),
    _ => Some(Per::Week),
  }
}

fn unit(caps: &Captures, i: usize) -> Option<f64> {
  caps.get(i).map(|m| match m.as_str() {
    "k" | "K" | "千" => 1_000.0,
    _ => 10_000.0,
  })
}

/// An amount, with or without thousands separators (`3,000`).
fn number(caps: &Captures, i: usize) -> Option<f64> {
  caps
    .get(i)
    .and_then(|m| m.as_str().replace(',', "").parse().ok())
}

/// The ISO code of a sign, code or word; the yuan (`¥`, `RMB`, `元`) and
/// anything unknown are `CNY`.
fn currency(symbol: &str) -> &'static str {
  match symbol.to_uppercase().as_str() {
    "$" | "US$" | "$US" | "美元" | "美金" => "USD",
    "HK$" => "HKD",
    "CA$" => "CAD",
    "A$" | "AU$" => "AUD",
    "NZ$" => "NZD",
    "MX$" => "MXN",
    "R$" => "BRL",
    "€" => "EUR",
    "£" => "GBP",
    "₹" => "INR",
    "₩" => "KRW",
    code => CODES
      .iter()
      .find(|c| **c == code && code != "RMB")
      .copied()
      .unwrap_or("CNY"),
  }
}

/// Full-width forms of the characters amounts are written with.
fn normalize(text: &str) -> String {
  text
    .chars()
    .map(|c| match c {
      '＄' => '$',
      '￥' => '¥',
      '～' | '〜' => '~',
      '－' | '—' | '–' => '-',
      '＋' => '+',
      '／' => '/',
      '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
      'Ｋ' | 'ｋ' => 'K',
      'Ｗ' | 'ｗ' => 'W',
      other => other,
    })
    .collect()
}
