//! Jobs as arguments: a job's link on any site ATP reads (what lists
//! print), or its id on the site `--site` names.
//!
//! - Feishu Hire: `<site>/<website>/position/<id>/detail`;
//! - Tencent: `join.qq.com/post_detail.html?postid=<id>`,
//!   `careers.tencent.com/jobdesc.html?postId=<id>`;
//! - Alibaba: `campus-talent.alibaba.com/campus/position/<id>`,
//!   `talent.alibaba.com/off-campus/position-detail?positionId=<id>`;
//! - Meituan: `zhaopin.meituan.com/web/position/detail?jobUnionId=<id>`;
//! - Kuaishou: `campus.kuaishou.cn/recruit/campus/e/#/campus/job-info/<id>`;
//! - Beisen: `<site>/<campus|social|intern>/detail?jobAdId=<id>`;
//! - Moka: `<site>#/job/<id>`.

use atp_core::{Error, Result};
use url::Url;

use crate::api::Api;
use crate::jobs::Kind;
use crate::sites::{self, Site, Vendor};

/// A job on a site.
#[derive(Debug, Clone)]
pub struct JobRef {
  pub site: Site,
  pub id: String,
}

/// A job's link, or its id on the `--site`.
pub async fn job(api: &Api, arg: &str) -> Result<JobRef> {
  let arg = arg.trim();
  let Some(url) = sites::link(arg) else {
    let site = sites::current(api).await?;
    return Ok(JobRef {
      site,
      id: arg.to_owned(),
    });
  };
  let mut site = sites::resolve(api, url.as_str()).await?;
  let Some(id) = id_in(&site, &url) else {
    return Err(
      Error::input(format!("{arg} is a career site, not one of its jobs")).with_hint(format!(
        "list its jobs with `atp careers jobs --site '{arg}'`"
      )),
    );
  };
  if site.vendor == Vendor::Beisen {
    let first = url.path().trim_matches('/').split('/').next();
    site.kind = site.kind.or(first.and_then(Kind::from_path));
  }
  Ok(JobRef { site, id })
}

/// A parameter of the query, by its name in any case (`postid`, `postId`).
fn param(url: &Url, name: &str) -> Option<String> {
  url
    .query_pairs()
    .find(|(k, _)| k.eq_ignore_ascii_case(name))
    .map(|(_, v)| v.trim().to_owned())
    .filter(|v| !v.is_empty())
}

/// The segment after `marker` in a path (`/position/<id>`).
fn after(path: &str, marker: &str) -> Option<String> {
  let mut parts = path.split(['/', '?']).filter(|p| !p.is_empty());
  parts.find(|p| *p == marker)?;
  parts.next().map(str::to_owned)
}

/// The job's id in a link of its site.
fn id_in(site: &Site, url: &Url) -> Option<String> {
  let fragment = url.fragment().unwrap_or_default();
  let id = match site.vendor {
    Vendor::Feishu => after(url.path(), "position").filter(|id| id != "list"),
    Vendor::Tencent => param(url, "postid"),
    Vendor::Alibaba => param(url, "positionId").or_else(|| after(url.path(), "position")),
    Vendor::Meituan => param(url, "jobUnionId"),
    Vendor::Kuaishou => after(fragment, "job-info"),
    Vendor::Beisen => param(url, "jobAdId"),
    Vendor::Moka => after(fragment, "job"),
  };
  id.filter(|id| {
    id.bytes()
      .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
  })
}
