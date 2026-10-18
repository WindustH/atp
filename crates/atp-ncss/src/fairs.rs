//! Job fairs (专场招聘): the site's own online fairs
//! (`/student/jobfair/fairList`, 中心站点招聘会), the fairs held around the
//! country (`/student/jobfair/placefair`, 各地招聘会), the fairs of the
//! 24365 campaign (`/student/24365/fairlist`), the companies of a fair
//! (`/student/jobfair/corp`). Fairs of other sites link there.
//!
//! The counts of a fair's companies, jobs and openings come from
//! `/student/jobfair/fairCount`, in the order of the fairs asked for.

use atp_core::{Page, PageReq, Post, Result, User, Value, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

/// Fairs per page.
const PAGE: u64 = 16;
/// Companies per page of a fair.
const COMPANIES: u64 = 16;

/// `fairs --list`: which fairs.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FairList {
  /// The site's own online fairs (中心站点招聘会, the default)
  Online,
  /// Fairs held around the country (各地招聘会)
  Local,
  /// The fairs of the 24365 campaign (全国网络联合招聘)
  #[value(name = "24365")]
  Campaign,
}

impl FairList {
  fn path(self) -> &'static str {
    match self {
      Self::Online => "/student/jobfair/fairList",
      Self::Local => "/student/jobfair/placefair",
      Self::Campaign => "/student/24365/fairlist",
    }
  }

  fn referer(self) -> &'static str {
    match self {
      Self::Online => "/student/jobfair/jobfairs.html?type=wl",
      Self::Local => "/student/jobfair/jobfairs.html?type=gd",
      Self::Campaign => "/student/24365",
    }
  }
}

/// One page of fairs; the 24365 campaign's come in one page.
pub async fn list(api: &Api, which: FairList, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let params = match which {
    FairList::Campaign => vec![("tab", String::new()), ("limit", "50".to_owned())],
    _ => vec![("offset", page.to_string()), ("limit", PAGE.to_string())],
  };
  let v = api.json(which.path(), &params, which.referer()).await?;
  let mut fairs: Vec<Post> = v.list("jobList").iter().filter_map(parse::fair).collect();
  counts(api, &mut fairs, which.referer()).await;
  if matches!(which, FairList::Campaign) {
    return Ok(Page::last(fairs));
  }
  let total = v.u64("total").unwrap_or(0);
  let more = !fairs.is_empty() && page * PAGE < total;
  Ok(Page::new(fairs, more.then(|| (page + 1).to_string())))
}

/// The companies, jobs and openings of the site's own fairs, as the page
/// fills them in; left out when the call fails.
async fn counts(api: &Api, fairs: &mut [Post], referer: &str) {
  let own: Vec<usize> = (0..fairs.len())
    .filter(|&i| !fairs[i].extra.contains_key("elsewhere"))
    .collect();
  if own.is_empty() {
    return;
  }
  let ids: String = own.iter().map(|&i| format!("{},", fairs[i].id)).collect();
  let params = [("fairIds", ids)];
  let Ok(v) = api
    .json("/student/jobfair/fairCount", &params, referer)
    .await
  else {
    return;
  };
  let rows: &[Value] = v.as_array().map_or(&[], Vec::as_slice);
  for (&i, row) in own.iter().zip(rows) {
    parse::counts(
      &mut fairs[i].extra,
      row,
      ["corpNums", "jobNums", "employNums"],
    );
  }
}

/// One page of the companies of a fair, those whose name contains `name`
/// when given.
pub async fn companies(
  api: &Api,
  arg: &str,
  name: Option<&str>,
  req: &PageReq,
) -> Result<Page<User>> {
  let fair = refs::fair(arg)?;
  let page = req.number_or(1);
  let params = [
    ("fairId", fair.clone()),
    ("corpName", name.unwrap_or_default().to_owned()),
    ("limit", COMPANIES.to_string()),
    ("offset", page.to_string()),
  ];
  let referer = format!("/student/jobfair/fairdetails.html?fairId={fair}");
  let v = api.data("/student/jobfair/corp", &params, &referer).await?;
  let items: Vec<User> = v
    .list("list")
    .iter()
    .filter_map(parse::fair_company)
    .collect();
  let pages = v.u64("pagenation.total").unwrap_or(0);
  let more = !items.is_empty() && page < pages;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}
