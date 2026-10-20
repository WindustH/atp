//! Your reading statistics (阅读数据) for a week, a month, a year or all
//! time, from the Agent Gateway (`/readdata/detail`): reading time, days,
//! the books read longest, and what you prefer (categories, authors,
//! publishers, hours of the day). Durations are seconds.

use atp_core::{Result, Value, ValueExt, json};

use crate::api::Api;

/// `--period` and the gateway's `mode`.
pub const PERIODS: &[(&str, &str)] = &[
  ("week", "weekly"),
  ("month", "monthly"),
  ("year", "annually"),
  ("all", "overall"),
];

pub async fn reading(api: &Api, period: &str, at: Option<i64>) -> Result<Value> {
  let gw = api.require_gateway("reading")?;
  let mode = PERIODS
    .iter()
    .find(|(name, _)| *name == period)
    .map_or("monthly", |(_, mode)| *mode);
  let mut params = vec![("mode", json!(mode))];
  if let Some(at) = at {
    params.push(("baseTime", json!(at)));
  }
  let v = gw.call("/readdata/detail", &params).await?;
  Ok(summary(period, &v))
}

fn summary(period: &str, v: &Value) -> Value {
  let start = v.i64("baseTime").and_then(atp_core::text::from_secs);
  json!({
    "period": period,
    "from": start.map(|t| t.to_string()),
    "read_days": v.u64("readDays"),
    "read_seconds": v.u64("totalReadTime"),
    "day_average_seconds": v.u64("dayAverageReadTime"),
    "change_from_previous": v.f64("compare"),
    "read_rate": v.f64("readRate"),
    "prefer_time": v.str("preferTimeWord"),
    "prefer_category": v.str("preferCategoryWord"),
    "longest": v.list("readLongest").iter().map(longest).collect::<Vec<_>>(),
    "counts": v.list("readStat").iter().map(|s| json!({
      "stat": s.str("stat"),
      "count": s.str("counts"),
    })).collect::<Vec<_>>(),
    "categories": v.list("preferCategory").iter().map(|c| json!({
      "category": c.str("categoryTitle"),
      "seconds": c.u64("readingTime"),
      "books": c.u64("readingCount"),
    })).collect::<Vec<_>>(),
    "authors": v.list("preferAuthor").iter().map(|a| json!({
      "author": a.str("name"),
      "time": a.str("readTime"),
      "books": a.u64("count"),
    })).collect::<Vec<_>>(),
    "publishers": v.list("preferPublisher").iter().map(|p| json!({
      "publisher": p.str("name"),
      "books": p.u64("count"),
    })).collect::<Vec<_>>(),
    "hours": hours(v.list("preferTime")),
  })
}

/// A book (or audio album) read longest.
fn longest(entry: &Value) -> Value {
  let album = entry.at("albumInfo");
  let (title, author) = if album.is_object() {
    (album.str("name"), album.str("authorName"))
  } else {
    (entry.str("book.title"), entry.str("book.author"))
  };
  json!({
    "book": entry.str("book.bookId").or_else(|| album.str("albumId")),
    "title": title,
    "author": author,
    "seconds": entry.u64("readTime"),
    "tags": entry.list("tags"),
  })
}

/// Reading time by hour of the day: the gateway lists 24 values from
/// 06:00 on.
fn hours(values: &[Value]) -> Value {
  if values.len() != 24 {
    return Value::Null;
  }
  let rows: Vec<Value> = values
    .iter()
    .enumerate()
    .map(|(i, seconds)| {
      json!({
        "hour": format!("{:02}:00", (6 + i) % 24),
        "seconds": seconds.as_u64().or_else(|| seconds.as_f64().map(|s| s as u64)),
      })
    })
    .collect();
  Value::Array(rows)
}
