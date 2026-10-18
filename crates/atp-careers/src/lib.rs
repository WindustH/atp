//! Companies' own career sites for all-the-platforms, through the requests
//! their pages make: the big companies by a short name (ByteDance on
//! Feishu Hire, Tencent, Alibaba, Meituan, Kuaishou) and any site whose
//! vendor ATP recognizes from its link (Feishu Hire, Beisen, Moka;
//! [`sites`]). Each vendor is a backend of its own ([`feishu`],
//! [`tencent`], [`alibaba`], [`meituan`], [`kuaishou`], [`beisen`],
//! [`moka`]) behind one job search and job page ([`jobs`]), with the
//! codes of a site's filters (batches, cities, categories).
//!
//! The site is `--site` (`Ctx::site`), except for job links, which name
//! their site. Everything is read without an account; applying happens on
//! the site.

mod alibaba;
mod api;
mod beisen;
mod extra;
mod feishu;
mod jobs;
mod kuaishou;
mod meituan;
mod moka;
mod parse;
mod refs;
mod sites;
mod tencent;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use jobs::Kind;

use std::time::Duration;

use atp_core::{
  Cap, Choices, Ctx, Data, Error, ErrorCode, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Query, Result, User,
};

use crate::api::Api;

pub struct Careers {
  api: Api,
}

impl Platform for Careers {
  const INFO: PlatformInfo = PlatformInfo {
    id: "careers",
    name: "Careers",
    aliases: &["career"],
    about: "Companies' own career sites: ByteDance, Tencent, Alibaba, Meituan, Kuaishou, and any site on Feishu Hire, Beisen or Moka",
    home: "https://jobs.bytedance.com/campus",
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[Cap::Search, Cap::Read],
    choices: Choices {
      search_filter: jobs::KINDS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(1500),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, sites, batches, cities, categories",
    guide: include_str!("../GUIDE.md"),
  };

  const SITE: Option<&'static str> = Some(
    "Career site: bytedance, tencent, alibaba, meituan, kuaishou, or a site's link (`atp careers sites`)",
  );

  type Extra = Command;

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "ATP reads career sites without an account; each company has its own, and applying happens on its site",
    ))
  }

  fn logged_in(&self) -> bool {
    false
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    let site = sites::current(&self.api).await?;
    jobs::search(&self.api, &site, &jobs::search_filter(query), page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    jobs::read(&self.api, post).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
