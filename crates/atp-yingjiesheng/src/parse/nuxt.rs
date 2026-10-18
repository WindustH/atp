//! What the mobile site's page tells of a talk. Its data is a Nuxt script,
//! `window.__NUXT__=(function(a,b,…){return {…xjhInfo:{…,startTime:1792301400,
//! endTime:d,…}…}}(x,y,…))`: values used more than once are passed in as
//! arguments and named by the parameters. Only the few fields of
//! `xjhInfo` ATP needs are read, each a literal or a parameter; the
//! script is never run.

use std::collections::HashMap;

use atp_core::text::{from_secs, html_to_text};
use atp_core::{Post, Value, json};

const SCRIPT: &str = "window.__NUXT__=(function(";

/// The script's parameters and the arguments they stand for.
fn arguments(html: &str) -> HashMap<String, Value> {
  let read = || -> Option<HashMap<String, Value>> {
    let script = &html[html.find(SCRIPT)? + SCRIPT.len()..];
    let script = &script[..script.find("</script>")?];
    let names = &script[..script.find(')')?];
    let call = &script[script.rfind("}(")? + 2..];
    let call = call.trim_end().trim_end_matches(';').trim_end_matches(')');
    let values: Vec<Value> =
      serde_json::from_str(&format!("[{}]", call.replace("void 0", "null"))).ok()?;
    Some(
      names
        .split(',')
        .map(|name| name.trim().to_owned())
        .zip(values)
        .collect(),
    )
  };
  read().unwrap_or_default()
}

/// A field of the talk's data (`xjhInfo`): a literal (number or string)
/// or a parameter of the script.
fn field(html: &str, args: &HashMap<String, Value>, key: &str) -> Option<Value> {
  let info = &html[html.find("xjhInfo:{")?..];
  let at = info
    .find(&format!(",{key}:"))
    .or_else(|| info.find(&format!("{{{key}:")))?
    + key.len()
    + 2;
  let rest = &info[at..];
  if rest.starts_with('"') {
    return serde_json::Deserializer::from_str(rest)
      .into_iter::<Value>()
      .next()?
      .ok();
  }
  let token: String = rest
    .chars()
    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
    .collect();
  if token.bytes().all(|b| b.is_ascii_digit()) {
    return token.parse::<i64>().ok().map(|n| json!(n));
  }
  args.get(&token).cloned()
}

/// What the mobile site's page adds to a talk read from the older page:
/// its start and end (`starts`, `ends`, and `created_at` the start), the
/// province and the company's industry; the city and the description
/// when the older page leaves them empty.
pub fn fill(html: &str, post: &mut Post) {
  let args = arguments(html);
  let text = |key: &str| {
    field(html, &args, key)
      .and_then(|v| v.as_str().map(str::to_owned))
      .filter(|v| !v.trim().is_empty())
  };
  let time = |key: &str| {
    field(html, &args, key)
      .and_then(|v| v.as_i64())
      .and_then(from_secs)
  };
  let (starts, ends) = (time("startTime"), time("endTime"));
  if let Some(starts) = starts {
    post.created_at = Some(starts);
    post.extra.insert("starts".into(), json!(starts));
  }
  if let Some(ends) = ends.filter(|e| starts.is_none_or(|s| *e > s)) {
    post.extra.insert("ends".into(), json!(ends));
  }
  for (key, path) in [("province", "areaName"), ("industry", "industryName")] {
    if let Some(value) = text(path) {
      post.extra.insert(key.into(), json!(value));
    }
  }
  if !post.extra.contains_key("city")
    && let Some(city) = text("cityName")
  {
    post.extra.insert("city".into(), json!(city));
  }
  if post.text.is_none() {
    post.text = text("detail")
      .map(|detail| html_to_text(&detail))
      .filter(|t| !t.is_empty());
  }
}
