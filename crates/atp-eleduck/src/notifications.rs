//! Messages (`/messages`, by the site's tabs) and their unread counts
//! (`/messages/read`).

use std::collections::BTreeMap;

use atp_core::{Notification, Page, PageReq, Result, ValueExt};

use crate::api::Api;
use crate::parse;

/// `notifications --type`, the site's tabs: 留言与回复 (the default), 电量提示,
/// 谁看过我, 社区通知, 招聘通知.
pub const KINDS: &[&str] = &["replies", "credits", "views", "system", "jobs"];

/// The site's name of a tab (`category`).
fn category(kind: &str) -> &'static str {
  match kind {
    "credits" => "charge",
    "views" => "profile",
    "system" => "system",
    "jobs" => "jobs_channel",
    _ => "content",
  }
}

/// ATP's name of one of the site's tabs.
fn kind(category: &str) -> &str {
  match category {
    "content" => "replies",
    "charge" => "credits",
    "profile" => "views",
    "jobs_channel" => "jobs",
    other => other,
  }
}

pub async fn list(api: &Api, kind: Option<&str>, req: &PageReq) -> Result<Page<Notification>> {
  let params = [
    ("page", req.number_or(1).to_string()),
    ("category", category(kind.unwrap_or("replies")).to_owned()),
  ];
  let v = api.get_own("/messages", &params).await?;
  let items = v.list("messages").iter().map(parse::notification).collect();
  Ok(Page::new(items, parse::next_page(&v)))
}

/// Unread messages per tab, and in all.
pub async fn unread(api: &Api) -> Result<BTreeMap<String, u64>> {
  let v = api.get_own("/messages/read", &[]).await?;
  let mut counts = BTreeMap::new();
  if let Some(tabs) = v.at("category").as_object() {
    for (name, n) in tabs {
      if let Some(n) = n.as_u64() {
        counts.insert(kind(name).to_owned(), n);
      }
    }
  }
  let total = v.u64("total").unwrap_or_else(|| counts.values().sum());
  counts.insert("total".into(), total);
  Ok(counts)
}
