//! Boards (版块): the sections, their boards and sub-boards
//! (`/api/forums`, kept a day in the cache), boards found by number, link
//! or name, and the thread types (分类) a board sorts its threads by.

use std::time::Duration;

use atp_core::{Collection, Error, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

const CACHE_KEY: &str = "1point3acres-boards";
const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);

/// Every section and board the site lists, from the cache or the API.
/// Boards the site has turned off (`status` 0) are left out.
async fn all(api: &Api) -> Result<Vec<Value>> {
  let store = &api.ctx.store;
  if let Some(Value::Array(list)) = store.cache_get::<Value>(CACHE_KEY, CACHE_TTL) {
    return Ok(list);
  }
  let v = api.get("/api/forums", &[]).await?;
  let list: Vec<Value> = v
    .list("forums")
    .iter()
    .filter(|f| f.u64("status") != Some(0))
    .cloned()
    .collect();
  if !list.is_empty() {
    store.cache_put(CACHE_KEY, &Value::Array(list.clone()));
  }
  Ok(list)
}

fn name(board: &Value) -> String {
  board
    .str("name")
    .map(|n| parse::plain(&n))
    .unwrap_or_default()
}

/// The boards right under a section or board.
fn children(list: &[Value], fid: u64) -> impl Iterator<Item = &Value> {
  list.iter().filter(move |f| f.u64("fup") == Some(fid))
}

/// Each section followed by its boards, each board by its sub-boards,
/// in the site's order; descriptions name where each one sits.
pub async fn list(api: &Api) -> Result<Vec<Collection>> {
  let list = all(api).await?;
  let mut sections: Vec<&Value> = list
    .iter()
    .filter(|f| f.str("type").as_deref() == Some("group"))
    .collect();
  sections.sort_by_key(|f| f.i64("displayorder").unwrap_or_default());
  let mut out = Vec::new();
  for section in sections {
    let fid = section.u64("fid").unwrap_or_default();
    let path = name(section);
    out.push(parse::board_entry(section, None));
    for board in children(&list, fid) {
      let board_path = format!("{path} › {}", name(board));
      out.push(parse::board_entry(board, Some(path.clone())));
      let id = board.u64("fid").unwrap_or_default();
      for sub in children(&list, id) {
        out.push(parse::board_entry(sub, Some(board_path.clone())));
      }
    }
  }
  Ok(out)
}

/// The board an argument names: a number or link, else a board's name as
/// the site shows it; with the site's entry for it where `boards` has one
/// (a number it lacks is still tried). Sections hold no threads of their
/// own.
pub async fn find(api: &Api, arg: &str) -> Result<(u64, Option<Value>)> {
  let list = all(api).await?;
  let wanted = arg.trim();
  let number = refs::board(wanted);
  let found = match number {
    Some(fid) => list.iter().find(|f| f.u64("fid") == Some(fid)),
    None => list
      .iter()
      .find(|f| name(f).eq_ignore_ascii_case(wanted))
      .or_else(|| list.iter().find(|f| name(f).contains(wanted))),
  };
  let Some(board) = found else {
    return match number {
      Some(fid) => Ok((fid, None)),
      None => Err(Error::not_found(format!(
        "1Point3Acres has no board {wanted} (`boards` lists them)"
      ))),
    };
  };
  if board.str("type").as_deref() == Some("group") {
    let fid = board.u64("fid").unwrap_or_default();
    let names: Vec<String> = children(&list, fid)
      .map(|b| format!("{} ({})", name(b), b.u64("fid").unwrap_or_default()))
      .collect();
    return Err(Error::input(format!(
      "{} is a section; pick one of its boards: {}",
      name(board),
      names.join(", ")
    )));
  }
  Ok((board.u64("fid").unwrap_or_default(), Some(board.clone())))
}

/// The thread type (分类) of a board a value names: its number or name.
pub fn thread_type(board: &Value, value: &str) -> Result<u64> {
  let value = value.trim();
  let types = board.at("forum_field.thread_types");
  let types: Vec<(String, String)> = types
    .as_object()
    .map(|map| {
      map
        .iter()
        .filter_map(|(id, name)| Some((id.clone(), parse::plain(name.as_str()?))))
        .collect()
    })
    .unwrap_or_default();
  let found = types
    .iter()
    .find(|(id, name)| id == value || name.eq_ignore_ascii_case(value))
    .or_else(|| types.iter().find(|(_, name)| name.contains(value)));
  if let Some(id) = found.and_then(|(id, _)| id.parse().ok()) {
    return Ok(id);
  }
  let names: Vec<&str> = types.iter().map(|(_, name)| name.as_str()).collect();
  Err(Error::input(if names.is_empty() {
    format!("{} sorts its threads by no type", name(board))
  } else {
    format!(
      "{} has no type {value} (one of: {})",
      name(board),
      names.join(", ")
    )
  }))
}
