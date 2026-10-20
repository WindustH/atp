//! The pins of guests' lists. Pinterest names the pins it lists to a
//! guest (the search) by ids of their own (`e1_…`), which open only within
//! the visit that listed them. ATP keeps those rows for a day, the newest
//! hundred, so `read` and `download` of such a pin answer from the list.

use std::collections::HashSet;
use std::time::Duration;

use atp_core::{Value, ValueExt};

use crate::api::Api;

/// The cache entry, how long and how many rows it keeps.
const KEY: &str = "listed-pins";
const KEPT_FOR: Duration = Duration::from_secs(86_400);
const MAX: usize = 100;

/// Whether an id is one of those Pinterest gives guests' lists.
pub fn guest_id(id: &str) -> bool {
  id.starts_with("e1_")
}

/// Keep the rows of a list whose pins have guests' ids, newest first.
pub fn remember(api: &Api, rows: &[&Value]) {
  let fresh: Vec<Value> = rows
    .iter()
    .filter(|r| r.str("id").is_some_and(|id| guest_id(&id)))
    .map(|r| (*r).clone())
    .collect();
  if fresh.is_empty() {
    return;
  }
  let ids: HashSet<String> = fresh.iter().filter_map(|r| r.str("id")).collect();
  let kept: Vec<Value> = api.ctx.store.cache_get(KEY, KEPT_FOR).unwrap_or_default();
  let mut all = fresh;
  all.extend(
    kept
      .into_iter()
      .filter(|r| r.str("id").is_none_or(|id| !ids.contains(&id))),
  );
  all.truncate(MAX);
  api.ctx.store.cache_put(KEY, &all);
}

/// A pin of a guest's list, as it was listed.
pub fn row(api: &Api, id: &str) -> Option<Value> {
  let kept: Vec<Value> = api.ctx.store.cache_get(KEY, KEPT_FOR)?;
  kept
    .into_iter()
    .find(|r| r.str("id").as_deref() == Some(id))
}
