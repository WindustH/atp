//! The 校招 schedule (校招日程, `/np-api/u/school-schedule/list-card`, 20
//! entries a page): companies' campus hiring by batch, with the page's
//! tabs and filters; subscribing to a company's entry (收藏, `/sns/follow`
//! with the company as a topic) puts it in the 我的收藏 tab.

use atp_core::{Action, Page, PageReq, Post, Result, ValueExt};

use crate::api::{Api, Body, Route};
use crate::{parse, refs};

/// Entries per page, as the page asks for them.
const PAGE: u64 = 20;
/// The 我的收藏 tab.
const SUBSCRIBED: u8 = 5;
/// The follow type of companies.
const TOPIC: &str = "4";

/// What the schedule is narrowed to, in the site's codes.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  pub query: String,
  /// `tab`: 1 推荐, 2 即将截止, 3 最近更新, 4 补招, 5 我的收藏, 6 专场.
  pub tab: u8,
  pub cities: Vec<String>,
  /// `careerJobId`.
  pub career: Option<String>,
  pub industries: Vec<String>,
  /// `propertyId`: the type of company.
  pub company_type: Option<String>,
  /// `batchId`.
  pub batch: Option<String>,
}

fn form(filter: &Filter, page: u64) -> Vec<(&'static str, String)> {
  let mut form = vec![
    ("query", filter.query.clone()),
    ("tab", filter.tab.to_string()),
    ("page", page.to_string()),
    ("pageSize", PAGE.to_string()),
  ];
  // 我的收藏 takes no filters.
  if filter.tab == SUBSCRIBED {
    return form;
  }
  form.extend(filter.cities.iter().map(|c| ("cities", c.clone())));
  form.extend(filter.career.iter().map(|c| ("careerJobId", c.clone())));
  form.extend(filter.industries.iter().map(|i| ("industryIds", i.clone())));
  form.extend(
    filter
      .company_type
      .iter()
      .map(|t| ("propertyId", t.clone())),
  );
  form.extend(filter.batch.iter().map(|b| ("batchId", b.clone())));
  form
}

pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  if filter.tab == SUBSCRIBED {
    api.require_login()?;
  }
  let page = req.number_or(1);
  let v = api
    .form(
      Route::Jobs,
      "/u/school-schedule/list-card",
      &form(filter, page),
    )
    .await?;
  let items: Vec<Post> = v.list("datas").iter().filter_map(parse::schedule).collect();
  let current = v.u64("currentPage").unwrap_or(page);
  let more = v.u64("totalPage").is_some_and(|pages| current < pages);
  let cursor = (more && !items.is_empty()).then(|| (current + 1).to_string());
  Ok(Page::new(items, cursor))
}

/// Subscribe to a company's schedule entry (收藏) or stop.
pub async fn subscribe(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let id = refs::company(arg)?;
  let path = if undo { "/sns/unfollow" } else { "/sns/follow" };
  let form = [("id", id.clone()), ("type", TOPIC.to_owned())];
  api.write(Route::Site, path, &[], Body::Form(&form)).await?;
  let action = if undo {
    "schedule-unsubscribe"
  } else {
    "schedule-subscribe"
  };
  Ok(Action::done(action, &id).with_url(refs::schedule_url(&id)))
}
