//! The pay a job title names (`25K-35K`, `$3000-$4000`, `年薪¥95W-150W`,
//! `17-20W/年`, `15-25K·14薪`), read conservatively: a range counts only
//! with a unit (K, W, 万, 千) or a currency, a single amount only with a
//! currency or a word for pay next to it (年薪, 月薪, 底薪, 最高 …). Annual pay is
//! turned into monthly pay (over the months paid, else 12); hourly and
//! daily pay keep only the text. Without a period, amounts from 100,000
//! up are taken as annual.

use std::sync::LazyLock;

use atp_core::json;
use atp_core::model::Extra;
use regex::{Captures, Regex};

static AMOUNT: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"(?i)(US\$|\$US|HK\$|\$|¥|€|£|USD|RMB)?\s*(\d{1,3}(?:,\d{3})+|\d+(?:\.\d+)?)\s*([kw万千])?(?:\s*(?:-|~|到|至)\s*(?:US\$|\$US|HK\$|\$|¥|€|£)?\s*(\d{1,3}(?:,\d{3})+|\d+(?:\.\d+)?)\s*([kw万千])?)?(\+|以上|起)?",
  )
  .unwrap()
});
static AFTER: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"(?i)^\s*(USD|RMB|CNY|美元|美金|元)?\s*(年薪|月薪|底薪|薪资|工资)?\s*(?:(/|每|一)\s*(月|年|天|日|小时|时|hour|day|month|year|mo|yr|h))?\s*(?:[·*x×]?\s*(\d{2})\s*薪)?",
  )
  .unwrap()
});

/// Words for pay right before an amount (年薪, 月薪, 底薪 … may also follow it).
const KEYWORDS: &[&str] = &[
  "年薪", "月薪", "底薪", "薪资", "薪酬", "工资", "待遇", "时薪", "日薪", "月入", "最高",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Period {
  Month,
  Year,
  /// Hourly or daily: no monthly figure.
  Shorter,
  Unknown,
}

/// The pay of a title.
#[derive(Debug, Clone, PartialEq)]
pub struct Pay {
  pub text: String,
  pub min: Option<f64>,
  pub max: Option<f64>,
  pub currency: &'static str,
  pub months: Option<u32>,
}

impl Pay {
  /// The shared job keys: `salary`, `salary_min` / `salary_max` (per
  /// month), `salary_currency`, `salary_months`.
  pub fn fill(&self, extra: &mut Extra) {
    extra.insert("salary".into(), json!(self.text));
    if let Some(min) = self.min {
      extra.insert("salary_min".into(), json!(min.round() as u64));
    }
    if let Some(max) = self.max {
      extra.insert("salary_max".into(), json!(max.round() as u64));
    }
    if self.min.is_some() || self.max.is_some() {
      extra.insert("salary_currency".into(), json!(self.currency));
    }
    if let Some(months) = self.months {
      extra.insert("salary_months".into(), json!(months));
    }
  }
}

/// Full-width forms of the characters amounts are written with.
fn normalize(title: &str) -> String {
  title
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

pub fn parse(title: &str) -> Option<Pay> {
  let text = normalize(title);
  AMOUNT
    .captures_iter(&text)
    .find_map(|caps| candidate(&text, &caps))
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

fn currency(symbol: &str) -> &'static str {
  match symbol.to_uppercase().as_str() {
    "$" | "US$" | "$US" | "USD" | "美元" | "美金" => "USD",
    "HK$" => "HKD",
    "€" => "EUR",
    "£" => "GBP",
    _ => "CNY",
  }
}

/// The pay one amount stands for, if the words around it say it is pay.
fn candidate(text: &str, caps: &Captures) -> Option<Pay> {
  let whole = caps.get(0)?;
  let before = &text[..whole.start()];
  let keyword = KEYWORDS
    .iter()
    .find(|k| before.trim_end().ends_with(*k))
    .copied();
  let after = AFTER.captures(&text[whole.end()..])?;
  let symbol = caps.get(1).or(after.get(1)).map(|m| m.as_str());
  let ranged = caps.get(4).is_some();
  let (unit_a, unit_b) = (unit(caps, 3), unit(caps, 5));
  let has_unit = unit_a.is_some() || unit_b.is_some();
  let named = keyword.or(after.get(2).map(|m| m.as_str()));
  let pay_words = symbol.is_some() || named.is_some();
  if !pay_words && !(ranged && has_unit) {
    return None;
  }
  let a = number(caps, 2)? * unit_a.or(unit_b).unwrap_or(1.0);
  if !ranged && a < 100.0 {
    return None;
  }
  let b = number(caps, 4).map(|b| b * unit_b.or(unit_a).unwrap_or(1.0));
  let (mut min, mut max) = match b {
    Some(b) => (Some(a), Some(b)),
    None if keyword == Some("最高") => (None, Some(a)),
    None if caps.get(6).is_some() => (Some(a), None),
    None => (Some(a), Some(a)),
  };
  let months = after.get(5).and_then(|m| m.as_str().parse::<u32>().ok());
  let period = period(named, after.get(4).map(|m| m.as_str()));
  let largest = max.or(min).unwrap_or_default();
  let per_month = match period {
    Period::Month => Some(1.0),
    Period::Year => Some(f64::from(months.unwrap_or(12))),
    Period::Unknown if largest >= 100_000.0 => Some(f64::from(months.unwrap_or(12))),
    Period::Unknown => Some(1.0),
    Period::Shorter => None,
  };
  match per_month {
    Some(divisor) => {
      min = min.map(|n| n / divisor);
      max = max.map(|n| n / divisor);
    }
    None => (min, max) = (None, None),
  }
  let start = keyword.map_or(whole.start(), |k| before.trim_end().len() - k.len());
  let end = whole.end() + after.get(0).map_or(0, |m| m.end());
  Some(Pay {
    text: text[start..end].trim().to_owned(),
    min,
    max,
    currency: currency(symbol.unwrap_or("¥")),
    months,
  })
}

fn period(keyword: Option<&str>, per: Option<&str>) -> Period {
  match keyword {
    Some("年薪") => return Period::Year,
    Some("月薪" | "月入" | "底薪") => return Period::Month,
    Some("时薪" | "日薪") => return Period::Shorter,
    _ => {}
  }
  match per.map(str::to_lowercase).as_deref() {
    Some("月" | "month" | "mo") => Period::Month,
    Some("年" | "year" | "yr") => Period::Year,
    Some(_) => Period::Shorter,
    None => Period::Unknown,
  }
}
