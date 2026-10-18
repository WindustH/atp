//! The pay as Liepin shows it on cards and job pages: `50-80k·24薪`,
//! `25-50k`, `1.5-2万`, `5000-8000元/月`, `20-40万/年`, `250-300元/天`,
//! `面议`. Monthly figures go into the shared job keys; annual pay is
//! divided by the months paid (12 unless `·N薪` says), and daily or
//! hourly pay keeps only the text.

use std::sync::LazyLock;

use atp_core::json;
use atp_core::model::Extra;
use regex::Regex;

static PAY: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"(?i)^(年薪)?\s*(\d+(?:\.\d+)?)\s*([kw万千])?\s*(?:[-~至到]\s*(\d+(?:\.\d+)?)\s*([kw万千])?)?\s*(元)?\s*(?:/\s*(月|年|天|日|时|小时|周))?\s*(以上|起)?\s*(?:[·•*x×]\s*(\d{1,2})\s*薪)?",
  )
  .expect("valid pattern")
});

#[derive(Debug, Clone, Copy, PartialEq)]
enum Period {
  Month,
  Year,
  /// Daily, hourly or weekly: no monthly figure.
  Shorter,
}

/// What the pay text says.
#[derive(Debug, Clone, PartialEq)]
pub struct Pay {
  pub text: String,
  pub min: Option<f64>,
  pub max: Option<f64>,
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
      extra.insert("salary_currency".into(), json!("CNY"));
    }
    if let Some(months) = self.months {
      extra.insert("salary_months".into(), json!(months));
    }
  }
}

fn unit(text: Option<&str>) -> Option<f64> {
  text.map(|u| match u {
    "k" | "K" | "千" => 1_000.0,
    _ => 10_000.0,
  })
}

/// The pay of a card or page; `None` for an empty text.
pub fn parse(text: &str) -> Option<Pay> {
  let text = text.trim();
  if text.is_empty() {
    return None;
  }
  let mut pay = Pay {
    text: text.to_owned(),
    min: None,
    max: None,
    months: None,
  };
  let Some(caps) = PAY.captures(text) else {
    return Some(pay);
  };
  let number = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<f64>().ok());
  let (unit_a, unit_b) = (
    unit(caps.get(3).map(|m| m.as_str())),
    unit(caps.get(5).map(|m| m.as_str())),
  );
  let a = number(2).map(|n| n * unit_a.or(unit_b).unwrap_or(1.0));
  let b = number(4).map(|n| n * unit_b.or(unit_a).unwrap_or(1.0));
  pay.months = caps.get(9).and_then(|m| m.as_str().parse().ok());
  let period = match caps.get(7).map(|m| m.as_str()) {
    _ if caps.get(1).is_some() => Period::Year,
    Some("年") => Period::Year,
    Some("月") | None => Period::Month,
    Some(_) => Period::Shorter,
  };
  let divisor = match period {
    Period::Month => 1.0,
    Period::Year => f64::from(pay.months.unwrap_or(12)),
    Period::Shorter => return Some(pay),
  };
  // A bare number without a unit, a currency or a period is not pay.
  let named =
    unit_a.is_some() || unit_b.is_some() || caps.get(6).is_some() || caps.get(7).is_some();
  if !named {
    return Some(pay);
  }
  pay.min = a.map(|n| n / divisor);
  pay.max = match (b, caps.get(8)) {
    (Some(b), _) => Some(b / divisor),
    (None, Some(_)) => None,
    (None, None) => pay.min,
  };
  Some(pay)
}
