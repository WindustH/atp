//! Trains: the ticket query (the trains between two stations on a day,
//! with the seats left per class), a train's stops (its number found
//! through search.12306.cn, then `czxx/queryByTrainNo`) and a train's
//! fares between two of its stations (`leftTicket/queryTicketPrice`, with
//! the station numbers and seat letters of its ticket record).

use atp_core::{Error, Post, Result, Value, ValueExt};
use jiff::civil::Date;

use crate::api::{self, Api};
use crate::parse::{self, field};
use crate::refs::{self, TrainRef};
use crate::stations::Stations;

/// Which trains of the query to keep.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  /// First letters of the codes (`G`, `D` …); empty keeps all.
  pub kinds: Vec<char>,
  /// Only trains with a seat left in some class.
  pub available: bool,
}

impl Filter {
  fn keeps(&self, post: &Post) -> bool {
    let code = post
      .extra
      .get("code")
      .and_then(Value::as_str)
      .unwrap_or_default();
    let kind = code.chars().next().map(|c| c.to_ascii_uppercase());
    let kind_ok = self.kinds.is_empty() || kind.is_some_and(|k| self.kinds.contains(&k));
    let seats = post.extra.get("seats").and_then(Value::as_array);
    let left = |s: &Value| {
      let left = s.str("left").unwrap_or_default();
      left == "有" || left.parse::<u32>().is_ok_and(|n| n > 0)
    };
    kind_ok && (!self.available || seats.is_some_and(|s| s.iter().any(left)))
  }
}

/// `trains FROM TO`: the day's trains, in 12306's order.
pub async fn list(
  api: &Api,
  from: &str,
  to: &str,
  date: Date,
  filter: &Filter,
) -> Result<Vec<Post>> {
  let stations = Stations::load(api).await?;
  let (from, to) = (stations.resolve(from)?, stations.resolve(to)?);
  if from.code == to.code {
    return Err(Error::input(format!(
      "{} is both ends of the trip",
      from.name
    )));
  }
  let data = query(api, &from.code, &to.code, date).await?;
  let name_of = names(&data, &stations);
  Ok(
    data
      .list("result")
      .iter()
      .filter_map(Value::as_str)
      .filter_map(|line| parse::train(line, date, &name_of))
      .filter(|post| filter.keeps(post))
      .collect(),
  )
}

/// Station names of the answer's `map`, else of the station list.
fn names<'a>(data: &'a Value, stations: &'a Stations) -> impl Fn(&str) -> String + 'a {
  move |code: &str| {
    data
      .str(&format!("map.{code}"))
      .unwrap_or_else(|| stations.name_of(code))
  }
}

/// The ticket query's `data`, following 12306 once when it names another
/// path for the query (`c_url`).
async fn query(api: &Api, from: &str, to: &str, date: Date) -> Result<Value> {
  let params = vec![
    ("leftTicketDTO.train_date", date.to_string()),
    ("leftTicketDTO.from_station", from.to_owned()),
    ("leftTicketDTO.to_station", to.to_owned()),
    ("purpose_codes", "ADULT".to_owned()),
  ];
  for _ in 0..2 {
    let path = api.query_path();
    let v = api.otn(&path, params.clone()).await?;
    if let Some(moved) = v.str("c_url").filter(|c| c.trim_start_matches('/') != path) {
      api.set_query_path(&moved);
      continue;
    }
    return Ok(api::ok(v)?.at("data").clone());
  }
  Err(Error::upstream(
    "12306 moved its ticket query twice in a row",
  ))
}

/// A train's stops on a day.
pub async fn stops(api: &Api, train: &TrainRef, date: Date) -> Result<Post> {
  let found = lookup(api, &train.code, date).await?;
  let train_no = found.str("train_no").unwrap_or_default();
  let (from, to) = match (&train.from, &train.to) {
    (Some(from), Some(to)) => (from.clone(), to.clone()),
    _ => {
      let stations = Stations::load(api).await?;
      let end = |key: &str| -> Result<String> {
        let name = found.str(key).unwrap_or_default();
        Ok(stations.resolve(&name)?.code.clone())
      };
      (end("from_station")?, end("to_station")?)
    }
  };
  let params = vec![
    ("train_no", train_no),
    ("from_station_telecode", from.clone()),
    ("to_station_telecode", to.clone()),
    ("depart_date", date.to_string()),
  ];
  let v = api::ok(api.otn("czxx/queryByTrainNo", params).await?)?;
  let id = refs::train_id(&train.code, &from, &to, date);
  parse::stops(v.list("data.data"), &id, date)
    .ok_or_else(|| Error::not_found(format!("12306 lists no stops of {} on {date}", train.code)))
}

/// A train of a day by its code (search.12306.cn): its number and ends.
async fn lookup(api: &Api, code: &str, date: Date) -> Result<Value> {
  let params = vec![
    ("keyword", code.to_owned()),
    ("date", date.strftime("%Y%m%d").to_string()),
  ];
  let v = api.search("search/v1/train/search", params).await?;
  v.list("data")
    .iter()
    .find(|t| {
      t.str("station_train_code")
        .is_some_and(|c| c.eq_ignore_ascii_case(code))
    })
    .cloned()
    .ok_or_else(|| Error::not_found(format!("12306 runs no train {code} on {date}")))
}

/// A train's fares between two stations on a day, with the seats left.
pub async fn fares(api: &Api, train: &TrainRef, from: &str, to: &str, date: Date) -> Result<Post> {
  let stations = Stations::load(api).await?;
  let (from, to) = (stations.resolve(from)?, stations.resolve(to)?);
  let data = query(api, &from.code, &to.code, date).await?;
  let line = data
    .list("result")
    .iter()
    .filter_map(Value::as_str)
    .find(|line| line.split('|').nth(field::CODE) == Some(train.code.as_str()))
    .ok_or_else(|| {
      Error::not_found(format!(
        "12306 lists no {} from {} to {} on {date}",
        train.code, from.name, to.name
      ))
    })?;
  let post = parse::train(line, date, names(&data, &stations))
    .ok_or_else(|| Error::upstream("12306 sent a ticket record ATP cannot read"))?;
  let f: Vec<&str> = line.split('|').collect();
  let params = vec![
    ("train_no", f[field::TRAIN_NO].to_owned()),
    ("from_station_no", f[field::FROM_NO].to_owned()),
    ("to_station_no", f[field::TO_NO].to_owned()),
    ("seat_types", f[field::SEAT_TYPES].to_owned()),
    ("train_date", date.to_string()),
  ];
  let v = api::ok(api.otn("leftTicket/queryTicketPrice", params).await?)?;
  let fares = parse::fares(v.at("data"));
  if fares.is_empty() {
    return Err(Error::not_found(format!(
      "12306 gave no fares for {}",
      train.code
    )));
  }
  Ok(parse::fare(&post, &fares))
}
