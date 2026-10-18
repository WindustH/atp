//! Unread messages per kind (`/api/sparta/message/pc/unread/detail`, what
//! the header's bell counts).

use std::collections::BTreeMap;

use atp_core::{Result, ValueExt};

use crate::api::{Api, Route};

/// ATP's name of one of the site's counters.
fn kind(key: &str) -> String {
  match key {
    "systemNotice" => "system".into(),
    "likeCollect" => "likes".into(),
    "commentMessage" => "comments".into(),
    "followMessage" => "follows".into(),
    "privateMessage" => "messages".into(),
    "nowPickJobApply" => "jobs".into(),
    other => other.to_owned(),
  }
}

/// Unread messages per kind, and in all (`total`).
pub async fn unread(api: &Api) -> Result<BTreeMap<String, u64>> {
  let v = api
    .own(Route::Gateway, "/api/sparta/message/pc/unread/detail", &[])
    .await?;
  let mut counts = BTreeMap::new();
  if let Some(kinds) = v.as_object() {
    for (key, value) in kinds {
      if let Some(n) = value.u64("unreadCount") {
        counts.insert(kind(key), n);
      }
    }
  }
  Ok(counts)
}
