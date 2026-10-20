//! What the creator platform's publish form offers a post, as
//! Collections: hashtags it suggests, locations (POI) and official
//! activities.

use atp_core::text::from_unix;
use atp_core::{Collection, Extra, Value, ValueExt, json};

use super::put;

/// A hashtag the publish form suggests (`sug_list` of `challengesug`):
/// its id (`cid`) and views.
pub fn hashtag(h: &Value) -> Option<Collection> {
  let name = h.str("cha_name")?;
  Some(Collection {
    id: h.str("cid").unwrap_or_else(|| name.clone()),
    kind: "hashtag".into(),
    name,
    views: h.count("view_count"),
    raw: Some(h.clone()),
    ..Collection::default()
  })
}

/// A location (POI) of the publish form's location box: its address in
/// `description`, `extra.city` and `extra.distance` as Douyin words them.
pub fn poi(p: &Value) -> Option<Collection> {
  let id = p.str("poi_id")?;
  let mut extra = Extra::new();
  let text = |paths: &[&str]| p.first_str(paths).map(Value::from);
  put(
    &mut extra,
    "city",
    text(&["address_info.city_name", "city_name"]),
  );
  put(&mut extra, "distance", text(&["distance"]));
  Some(Collection {
    kind: "location".into(),
    name: p.str("poi_name").unwrap_or_default(),
    description: p.first_str(&[
      "address_info.simple_addr",
      "address_info.address",
      "address",
    ]),
    extra,
    raw: Some(p.clone()),
    id,
    ..Collection::default()
  })
}

/// An official activity (官方活动): when it runs and its hashtags in
/// `description`; `extra.heat`, `extra.topics` and `extra.topic_ids` (the
/// hashtags a post joins it through), `extra.end_time`, and its kind,
/// reward and category codes as Douyin gives them.
pub fn activity(a: &Value) -> Option<Collection> {
  let id = a.str("activity_id")?;
  let strings =
    |key: &str| -> Vec<String> { a.list(key).iter().filter_map(|t| t.str("")).collect() };
  let topics = strings("challenge");
  let mut extra = Extra::new();
  put(&mut extra, "heat", a.count("hot_score").map(Value::from));
  for (key, list) in [
    ("topics", &topics),
    ("topic_ids", &strings("challenge_ids")),
  ] {
    if !list.is_empty() {
      extra.insert(key.into(), json!(list));
    }
  }
  let end = a.i64("end_time").and_then(from_unix);
  put(
    &mut extra,
    "end_time",
    end.map(|t| Value::from(t.to_string())),
  );
  for key in ["activity_type", "reward_type", "query_tag"] {
    put(&mut extra, key, Some(a.at(key).clone()));
  }
  Some(Collection {
    kind: "activity".into(),
    name: a.first_str(&["activity_name", "title"]).unwrap_or_default(),
    description: activity_line(a, &topics),
    extra,
    raw: Some(a.clone()),
    id,
    ..Collection::default()
  })
}

/// `2026.10.01 ~ 2026.10.31 #topic`, as the activity picker shows it.
fn activity_line(a: &Value, topics: &[String]) -> Option<String> {
  let mut parts = Vec::new();
  let (start, end) = (a.str("show_start_time"), a.str("show_end_time"));
  if start.is_some() || end.is_some() {
    parts.push(format!(
      "{} ~ {}",
      start.unwrap_or_default(),
      end.unwrap_or_default()
    ));
  }
  parts.extend(topics.iter().map(|t| format!("#{t}")));
  Some(parts.join(" ")).filter(|line| !line.is_empty())
}
