//! 12306 payloads → posts: stations, trains of the ticket query (a
//! `|`-separated record each), a train's stops and a train's fares. The
//! words shown are 12306's own (seat classes, 有 / 无 / counts left).

use atp_core::text::from_beijing;
use atp_core::{Post, Price, Value, ValueExt, json};
use jiff::civil::Date;

use crate::refs;
use crate::stations::Station;

/// Seat classes in a ticket record: (field, name); a field holds `有`
/// (plenty), `无` (none), a count left, `*` (not on sale yet) or nothing
/// (no such class).
const SEATS: &[(usize, &str)] = &[
  (32, "商务座"),
  (25, "特等座"),
  (31, "一等座"),
  (30, "二等座"),
  (21, "高级软卧"),
  (23, "软卧/一等卧"),
  (33, "动卧"),
  (28, "硬卧/二等卧"),
  (24, "软座"),
  (29, "硬座"),
  (26, "无座"),
  (22, "其他"),
];

/// Seat letters of the fare query and their names.
const FARES: &[(&str, &str)] = &[
  ("A9", "商务座"),
  ("P", "特等座"),
  ("M", "一等座"),
  ("D", "优选一等座"),
  ("O", "二等座"),
  ("A6", "高级软卧"),
  ("A4", "软卧/一等卧"),
  ("I", "一等卧"),
  ("F", "动卧"),
  ("A3", "硬卧/二等卧"),
  ("J", "二等卧"),
  ("A2", "软座"),
  ("A1", "硬座"),
  ("WZ", "无座"),
  ("H", "其他"),
];

/// Fields of a ticket record ATP reads.
pub mod field {
  pub const STATUS: usize = 1;
  pub const TRAIN_NO: usize = 2;
  pub const CODE: usize = 3;
  pub const ORIGIN: usize = 4;
  pub const TERMINUS: usize = 5;
  pub const FROM: usize = 6;
  pub const TO: usize = 7;
  pub const DEPART: usize = 8;
  pub const ARRIVE: usize = 9;
  pub const DURATION: usize = 10;
  pub const BOOKABLE: usize = 11;
  pub const FROM_NO: usize = 16;
  pub const TO_NO: usize = 17;
  pub const SEAT_TYPES: usize = 35;
  /// Records shorter than this are not trains.
  pub const MIN: usize = 36;
}

pub fn station(s: &Station) -> Post {
  let mut post = Post {
    id: s.code.clone(),
    kind: "station".into(),
    title: Some(s.name.clone()),
    text: Some(format!("{} · {}", s.city, s.pinyin)),
    ..Post::default()
  };
  for (key, value) in [
    ("code", &s.code),
    ("pinyin", &s.pinyin),
    ("abbr", &s.abbr),
    ("city", &s.city),
  ] {
    post.extra.insert(key.into(), json!(value));
  }
  post
}

/// `06:26` → minutes.
fn minutes(clock: &str) -> Option<u32> {
  let (h, m) = clock.split_once(':')?;
  Some(h.parse::<u32>().ok()? * 60 + m.parse::<u32>().ok()?)
}

/// The seats of a record that the train has: `{name, left}`.
pub fn seats(f: &[&str]) -> Vec<Value> {
  SEATS
    .iter()
    .filter_map(|(i, name)| {
      let left = f.get(*i)?.trim();
      (!left.is_empty() && left != "--").then(|| json!({ "name": name, "left": left }))
    })
    .collect()
}

/// A train of the ticket query (one `|`-separated record); `name_of`
/// names telecodes.
pub fn train(line: &str, date: Date, name_of: impl Fn(&str) -> String) -> Option<Post> {
  let f: Vec<&str> = line.split('|').collect();
  if f.len() < field::MIN || f[field::CODE].is_empty() {
    return None;
  }
  let (code, from, to) = (f[field::CODE], f[field::FROM], f[field::TO]);
  let (depart, arrive, duration) = (f[field::DEPART], f[field::ARRIVE], f[field::DURATION]);
  let days = minutes(depart)
    .zip(minutes(duration))
    .map(|(d, l)| (d + l) / 1440)
    .filter(|d| *d > 0);
  let later = days.map(|d| format!("+{d}")).unwrap_or_default();
  let seats = seats(&f);
  let mut raw: Vec<&str> = f.clone();
  raw[0] = "";
  let mut post = Post {
    id: refs::train_id(code, from, to, date),
    kind: "train".into(),
    title: Some(format!(
      "{code} {} {depart} → {} {arrive}{later}",
      name_of(from),
      name_of(to)
    )),
    text: Some(train_line(f[field::STATUS], duration, &seats)),
    created_at: from_beijing(&format!("{date} {depart}")),
    raw: Some(json!(raw)),
    ..Post::default()
  };
  let extra = &mut post.extra;
  let fields = [
    ("code", json!(code)),
    ("train_no", json!(f[field::TRAIN_NO])),
    ("from", json!(name_of(from))),
    ("to", json!(name_of(to))),
    ("depart", json!(depart)),
    ("arrive", json!(arrive)),
    ("duration", json!(duration)),
    ("origin", json!(name_of(f[field::ORIGIN]))),
    ("terminus", json!(name_of(f[field::TERMINUS]))),
    ("bookable", json!(f[field::BOOKABLE] == "Y")),
    ("seats", json!(seats)),
  ];
  extra.extend(fields.into_iter().map(|(k, v)| (k.to_owned(), v)));
  if let Some(days) = days {
    extra.insert("days".into(), json!(days));
  }
  Some(post)
}

/// `历时 05:47 · 商务座 3 · 二等座 有`, led by a status other than 预订
/// (`列车停运`).
fn train_line(status: &str, duration: &str, seats: &[Value]) -> String {
  let status = status.trim();
  let mut parts: Vec<String> = Vec::new();
  if !status.is_empty() && status != "预订" {
    parts.push(status.to_owned());
  }
  parts.push(format!("历时 {duration}"));
  parts.extend(seats.iter().map(|s| {
    let name = s.str("name").unwrap_or_default();
    format!("{name} {}", s.str("left").unwrap_or_default())
  }));
  parts.join(" · ")
}

/// `----` and empty times are none.
fn time(v: &Value, key: &str) -> Option<String> {
  v.str(key).filter(|t| !t.starts_with("--"))
}

/// A train's stops (`queryByTrainNo` → `data.data`) as one post.
pub fn stops(list: &[Value], id: &str, date: Date) -> Option<Post> {
  let first = list.first()?;
  let code = first.str("station_train_code").unwrap_or_default();
  let from = first.str("start_station_name").unwrap_or_default();
  let to = first.str("end_station_name").unwrap_or_default();
  let lines: Vec<String> = list.iter().map(stop_line).collect();
  let stops: Vec<Value> = list
    .iter()
    .map(|s| {
      let [arrive, depart, stop] = times(s);
      json!({
        "no": s.str("station_no"),
        "station": s.str("station_name"),
        "arrive": arrive,
        "depart": depart,
        "stop": stop,
        "in_segment": s.bool("isEnabled"),
      })
    })
    .collect();
  let mut post = Post {
    id: id.to_owned(),
    kind: "train".into(),
    title: Some(format!("{code} {from} → {to}")),
    text: Some(lines.join("\n")),
    created_at: time(first, "start_time").and_then(|t| from_beijing(&format!("{date} {t}"))),
    raw: Some(Value::Array(list.to_vec())),
    ..Post::default()
  };
  post.extra.insert("code".into(), json!(code));
  if let Some(class) = first.str("train_class_name") {
    post.extra.insert("class".into(), json!(class));
  }
  post.extra.insert("stops".into(), json!(stops));
  Some(post)
}

/// Arrival, departure and stop time at a station; the terminus (an
/// arrival without a stop) has no departure, though 12306 repeats the
/// arrival there.
fn times(s: &Value) -> [Option<String>; 3] {
  let (arrive, stop) = (time(s, "arrive_time"), time(s, "stopover_time"));
  let terminus = arrive.is_some() && stop.is_none();
  let depart = time(s, "start_time").filter(|_| !terminus);
  [arrive, depart, stop]
}

/// `03 南京南 到 07:31 开 07:33 停 2分钟`.
fn stop_line(s: &Value) -> String {
  let mut line = format!(
    "{} {}",
    s.str("station_no").unwrap_or_default(),
    s.str("station_name").unwrap_or_default()
  );
  for (word, t) in ["到", "开", "停"].into_iter().zip(times(s)) {
    if let Some(t) = t {
      line.push_str(&format!(" {word} {t}"));
    }
  }
  line
}

/// A fare answer (`queryTicketPrice` → `data`): `¥553.0` per seat letter.
pub fn fares(v: &Value) -> Vec<(String, String, f64)> {
  let price = |letter: &str| {
    v.str(letter)
      .and_then(|p| p.trim_start_matches('¥').parse::<f64>().ok())
  };
  let mut fares: Vec<(String, String, f64)> = FARES
    .iter()
    .filter_map(|(letter, name)| Some(((*letter).to_owned(), (*name).to_owned(), price(letter)?)))
    .collect();
  // Letters not named above, still with a price.
  for (letter, _) in v.as_object().into_iter().flatten() {
    let known = FARES.iter().any(|(l, _)| l == letter);
    let letters = letter.starts_with(|c: char| c.is_ascii_uppercase());
    if let Some(p) = price(letter).filter(|_| !known && letters && letter != "OT") {
      fares.push((letter.clone(), letter.clone(), p));
    }
  }
  fares
}

/// A train's fares between two of its stations, with the seats left.
pub fn fare(train: &Post, fares: &[(String, String, f64)]) -> Post {
  let left = |name: &str| {
    train
      .extra
      .get("seats")
      .and_then(Value::as_array)
      .and_then(|seats| {
        seats
          .iter()
          .find(|s| s.str("name").as_deref() == Some(name))
      })
      .and_then(|s| s.str("left"))
  };
  let seats: Vec<Value> = fares
    .iter()
    .map(|(letter, name, price)| json!({ "code": letter, "name": name, "price": price, "left": left(name) }))
    .collect();
  let text: Vec<String> = fares
    .iter()
    .map(|(_, name, price)| format!("{name} {}", Price::cny(*price).display()))
    .collect();
  let lowest = fares
    .iter()
    .map(|(_, _, p)| *p)
    .filter(|p| *p > 0.0)
    .reduce(f64::min);
  let mut post = Post {
    kind: "fare".into(),
    text: Some(text.join(" · ")),
    price: lowest.map(|p| Price {
      label: Some("起".into()),
      ..Price::cny(p)
    }),
    raw: None,
    ..train.clone()
  };
  post.extra.insert("seats".into(), json!(seats));
  post
}
