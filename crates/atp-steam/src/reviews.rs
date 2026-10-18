//! Reviews (`/appreviews/<app>?json=1`), page after page through an
//! opaque cursor (`*` first): in one of Steam's orders, in one language
//! or all of them, positive or negative ones, by where the game was
//! bought.

use atp_core::{Comment, Error, Page, PageReq, Result, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// `comments --sort`: Steam's orders, and its two kinds of review in the
/// helpful order.
pub const SORTS: &[&str] = &["helpful", "recent", "updated", "positive", "negative"];
/// The most reviews Steam sends a page.
const PAGE_MAX: usize = 100;
/// The longest window of helpful reviews, in days.
pub const DAYS_MAX: u32 = 365;

/// Which reviews, in which order.
#[derive(Debug, Clone)]
pub struct Filter {
  /// `helpful`, `recent` or `updated`.
  pub sort: String,
  /// `all`, `positive` or `negative`.
  pub kind: String,
  /// Steam's name of a language (`english`, `schinese`), or `all`.
  pub language: String,
  /// `all`, `steam` or `other` (keys bought elsewhere).
  pub purchase: String,
  /// Helpful reviews of the last N days only.
  pub days: Option<u32>,
}

impl Filter {
  /// What `comments --sort` asks for, in every language.
  pub fn sorted(sort: Option<&str>) -> Self {
    let (sort, kind) = match sort {
      Some(kind @ ("positive" | "negative")) => ("helpful", kind),
      Some(sort) => (sort, "all"),
      None => ("helpful", "all"),
    };
    Self {
      sort: sort.into(),
      kind: kind.into(),
      language: "all".into(),
      purchase: "all".into(),
      days: None,
    }
  }

  fn params(&self) -> Vec<(&'static str, String)> {
    let filter = match self.sort.as_str() {
      "helpful" => "all",
      other => other,
    };
    let purchase = match self.purchase.as_str() {
      "other" => "non_steam_purchase",
      other => other,
    };
    let mut params = vec![
      ("json", "1".to_owned()),
      ("filter", filter.to_owned()),
      ("review_type", self.kind.clone()),
      ("language", self.language.to_ascii_lowercase()),
      ("purchase_type", purchase.to_owned()),
    ];
    if let Some(days) = self.days.filter(|_| filter == "all") {
      params.push(("day_range", days.min(DAYS_MAX).to_string()));
    }
    params
  }
}

/// One page of an app's reviews.
pub async fn list(api: &Api, arg: &str, filter: &Filter, page: &PageReq) -> Result<Page<Comment>> {
  let id = refs::app(arg)?;
  let mut params = filter.params();
  params.push(("num_per_page", page.size_within(PAGE_MAX).to_string()));
  params.push(("cursor", page.cursor.clone().unwrap_or_else(|| "*".into())));
  let v = reviews(api, id, params).await?;
  let app = id.to_string();
  let items: Vec<Comment> = v
    .list("reviews")
    .iter()
    .map(|review| parse::review(review, &app))
    .collect();
  let next = v
    .str("cursor")
    .filter(|cursor| !items.is_empty() && page.cursor.as_ref() != Some(cursor));
  Ok(Page::new(items, next))
}

/// The summary of an app's reviews in every language (`Very Positive`,
/// counts).
pub async fn summary(api: &Api, id: u64) -> Result<Value> {
  let params = vec![
    ("json", "1".to_owned()),
    ("language", "all".to_owned()),
    ("purchase_type", "all".to_owned()),
    ("num_per_page", "0".to_owned()),
  ];
  let v = reviews(api, id, params).await?;
  Ok(v.at("query_summary").clone())
}

/// `appreviews`; `l` is the language of the summary's words.
async fn reviews(api: &Api, id: u64, mut params: Vec<(&'static str, String)>) -> Result<Value> {
  params.push(("l", api.language()));
  let v = api.store(&format!("appreviews/{id}"), params).await?;
  if v.i64("success") != Some(1) {
    return Err(Error::upstream(format!(
      "Steam did not list the reviews of app {id}"
    )));
  }
  Ok(v)
}
