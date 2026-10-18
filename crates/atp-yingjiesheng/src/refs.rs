//! Talks and companies as arguments.
//!
//! - talks (宣讲会): their number (`6299612`), or a link holding it:
//!   `https://my.yingjiesheng.com/xjh-006-299-612.html` (what lists print:
//!   the number in groups of three, padded to nine digits) or the mobile
//!   site's `https://m.yingjiesheng.com/xuanjianghui/xjh_6299612`;
//! - companies: their number (`1213060`), or the link of their talks,
//!   `https://my.yingjiesheng.com/xuanjianghui_cid_1213060.html`.

use atp_core::{Error, Result};
use url::Url;

use crate::api::{MOBILE, MY};

/// A talk's page on the older site: the number padded to a multiple of
/// three digits (at least nine) and grouped by three.
pub fn talk_url(id: &str) -> String {
  let width = id.len().max(9).div_ceil(3) * 3;
  let padded = format!("{id:0>width$}");
  let groups: Vec<&str> = padded
    .as_bytes()
    .chunks(3)
    .filter_map(|chunk| std::str::from_utf8(chunk).ok())
    .collect();
  format!("{MY}/xjh-{}.html", groups.join("-"))
}

pub fn mobile_url(id: &str) -> String {
  format!("{MOBILE}/xuanjianghui/xjh_{id}")
}

pub fn company_url(id: &str) -> String {
  format!("{MY}/xuanjianghui_cid_{id}.html")
}

pub fn school_url(id: &str) -> String {
  format!("{MY}/xuanjianghui_school_{id}.html")
}

/// The last part of a link's path without `.html` (or the argument as
/// typed).
fn last_part(arg: &str) -> String {
  let arg = arg.trim();
  let path = match Url::parse(arg) {
    Ok(url) => url.path().to_owned(),
    Err(_) => arg.split(['#', '?']).next().unwrap_or(arg).to_owned(),
  };
  let last = path.trim_matches('/').rsplit('/').next().unwrap_or("");
  last.trim_end_matches(".html").to_owned()
}

fn number(text: &str) -> Option<String> {
  let trimmed = text.trim_start_matches('0');
  (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
    .then(|| if trimmed.is_empty() { "0" } else { trimmed }.to_owned())
}

/// A talk's number, from its link or as it is.
pub fn talk(arg: &str) -> Result<String> {
  let last = last_part(arg);
  let digits = if let Some(groups) = last.strip_prefix("xjh-") {
    groups.replace('-', "")
  } else {
    last.strip_prefix("xjh_").unwrap_or(&last).to_owned()
  };
  number(&digits).ok_or_else(|| {
    Error::input(format!(
      "not a Yingjiesheng talk: {arg} (expected an /xjh-…html link or the talk's number)"
    ))
  })
}

/// A company's number, from the link of its talks or as it is.
pub fn company(arg: &str) -> Result<String> {
  let last = last_part(arg);
  let digits = last.strip_prefix("xuanjianghui_cid_").unwrap_or(&last);
  number(digits).ok_or_else(|| {
    Error::input(format!(
      "not a Yingjiesheng company: {arg} (expected an /xuanjianghui_cid_….html link or the company's number)"
    ))
  })
}
