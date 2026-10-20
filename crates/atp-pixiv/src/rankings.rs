//! The rankings (`ranking.php?mode=…&format=json`): fifty artworks a page,
//! up to five hundred for the daily ranking. The rankings for adults
//! need a login whose settings allow them; ATP does not ask for them
//! without one.

use atp_core::{Error, Page, PageReq, Post, Result, ValueExt};

use crate::api::Api;
use crate::parse;

/// `hot --category`: the site's ranking modes; the first is the default.
pub const MODES: &[&str] = &[
  "daily",
  "weekly",
  "monthly",
  "rookie",
  "original",
  "daily_ai",
  "male",
  "female",
  "daily_r18",
  "weekly_r18",
  "male_r18",
  "female_r18",
  "daily_r18_ai",
  "r18g",
];

/// Whether a mode ranks works for adults.
fn for_adults(mode: &str) -> bool {
  mode.contains("r18")
}

pub async fn hot(api: &Api, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  let mode = category.unwrap_or(MODES[0]);
  if for_adults(mode) && !api.logged_in() {
    return Err(
      Error::auth(format!(
        "the {mode} ranking lists works for adults; ATP asks for it only with a login whose settings allow them"
      ))
      .with_hint(api.ctx.login_hint()),
    );
  }
  let no = page.number_or(1);
  let query = vec![
    ("mode", mode.to_owned()),
    ("p", no.to_string()),
    ("format", "json".to_owned()),
  ];
  let v = api.get_plain("/ranking.php", query).await?;
  let posts = v
    .list("contents")
    .iter()
    .filter(|row| api.logged_in() || !parse::restricted(row))
    .map(parse::ranked)
    .collect();
  let next = v.u64("next").filter(|n| *n > no).map(|n| n.to_string());
  Ok(Page::new(posts, next))
}
