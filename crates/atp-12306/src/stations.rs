//! Stations: 12306's list (`station_name.js`, kept a week), searched by
//! name, telecode, pinyin or city, and station arguments turned into
//! telecodes, the station codes the queries take.

use std::time::Duration;

use atp_core::{Error, Result};

use crate::api::Api;

const LIST: &str = "/otn/resources/js/framework/station_name.js";
const CACHE: &str = "station_name.js";
const TTL: Duration = Duration::from_secs(7 * 86_400);

#[derive(Debug, Clone)]
pub struct Station {
  /// `上海虹桥`.
  pub name: String,
  /// The telecode: `AOH`.
  pub code: String,
  /// `shanghaihongqiao`.
  pub pinyin: String,
  /// `shhq`.
  pub abbr: String,
  /// `上海`.
  pub city: String,
}

pub struct Stations(Vec<Station>);

impl Stations {
  /// The list, from the cache or from 12306.
  pub async fn load(api: &Api) -> Result<Self> {
    if let Some(bytes) = api.ctx.store.cache_get_bytes(CACHE, TTL) {
      let list = parse(&String::from_utf8_lossy(&bytes));
      if !list.is_empty() {
        return Ok(Self(list));
      }
    }
    let text = api.file(LIST).await?;
    let list = parse(&text);
    if list.is_empty() {
      return Err(Error::upstream("12306's station list has no stations"));
    }
    api.ctx.store.cache_put_bytes(CACHE, text.as_bytes());
    Ok(Self(list))
  }

  pub fn by_code(&self, code: &str) -> Option<&Station> {
    self.0.iter().find(|s| s.code == code)
  }

  /// A station's name, or the code when the list lacks it.
  pub fn name_of(&self, code: &str) -> String {
    self
      .by_code(code)
      .map_or_else(|| code.to_owned(), |s| s.name.clone())
  }

  /// Stations matching a keyword: exact matches first, then names and
  /// pinyin starting with it, then anything containing it.
  pub fn find(&self, keyword: &str) -> Vec<&Station> {
    let word = keyword.trim();
    let lower = word.to_lowercase();
    let rank = |s: &Station| -> Option<u8> {
      if s.name == word || s.code == word.to_uppercase() || s.pinyin == lower || s.abbr == lower {
        Some(0)
      } else if s.name.starts_with(word) || s.pinyin.starts_with(&lower) {
        Some(1)
      } else if s.name.contains(word) || s.city.contains(word) || s.pinyin.contains(&lower) {
        Some(2)
      } else {
        None
      }
    };
    let mut found: Vec<(u8, &Station)> = self
      .0
      .iter()
      .filter_map(|s| rank(s).map(|r| (r, s)))
      .collect();
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, s)| s).collect()
  }

  /// The station an argument names: a telecode (`AOH`), a name (`上海虹桥`,
  /// `上海站`), pinyin (`shanghaihongqiao`) or its initials (`shhq`). A
  /// city's own name is its main station (`上海` → `SHH`), with which 12306
  /// lists the trains of every station of the city.
  pub fn resolve(&self, arg: &str) -> Result<&Station> {
    let arg = arg.trim();
    let name = arg.strip_suffix('站').unwrap_or(arg);
    let lower = arg.to_lowercase();
    let exact = self
      .0
      .iter()
      .find(|s| s.code == arg || s.name == name)
      .or_else(|| self.0.iter().find(|s| s.pinyin == lower))
      .or_else(|| self.0.iter().find(|s| s.abbr == lower));
    if let Some(station) = exact {
      return Ok(station);
    }
    let like: Vec<&str> = self
      .find(name)
      .iter()
      .take(5)
      .map(|s| s.name.as_str())
      .collect();
    let hint = if like.is_empty() {
      "see `atp 12306 stations NAME`".to_owned()
    } else {
      format!("did you mean {}?", like.join(", "))
    };
    Err(Error::not_found(format!("12306 knows no station {arg}")).with_hint(hint))
  }
}

/// `var station_names ='@bjb|北京北|VAP|beijingbei|bjb|0|0357|北京|||@…';`:
/// records split by `@`, fields by `|` (initials, name, telecode, pinyin,
/// initials again, index, city code, city).
fn parse(text: &str) -> Vec<Station> {
  let (Some(start), Some(end)) = (text.find('\''), text.rfind('\'')) else {
    return Vec::new();
  };
  if end <= start {
    return Vec::new();
  }
  text[start + 1..end]
    .split('@')
    .filter_map(|record| {
      let f: Vec<&str> = record.split('|').collect();
      if f.len() < 8 || f[1].is_empty() || f[2].is_empty() {
        return None;
      }
      Some(Station {
        name: f[1].to_owned(),
        code: f[2].to_owned(),
        pinyin: f[3].to_owned(),
        abbr: f[4].to_owned(),
        city: f[7].to_owned(),
      })
    })
    .collect()
}
