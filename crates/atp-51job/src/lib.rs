//! 51job (前程无忧, 51job.com), the job site, for all-the-platforms. Its
//! search API answers only a real browser (Aliyun's WAF runs scripts in
//! front of it), so ATP reads 51job through a browser tab
//! ([`atp_core::tab`]): the site's own search page loads and makes the
//! requests ([`api`]). The job search with the site's filters, each card
//! with the job's whole description ([`jobs`], [`filters`]), and job and
//! company pages ([`pages`], [`users`]); without an account.

mod api;
mod extra;
mod filters;
mod jobs;
mod pages;
mod parse;
mod refs;
mod sign;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use filters::{CompanyType, Degree, Experience, JobType, Pay, Size};

use std::time::Duration;

use atp_core::{
  Cap, Choices, Ctx, Data, Error, ErrorCode, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Query, Result, User,
};

use crate::api::Api;

pub struct FiftyOneJob {
  api: Api,
}

impl Platform for FiftyOneJob {
  const INFO: PlatformInfo = PlatformInfo {
    id: "51job",
    name: "51job",
    aliases: &["qiancheng", "qcwy"],
    about: "51job (前程无忧): job search with the site's filters, jobs, companies; read through a browser",
    home: api::WE,
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[Cap::Search, Cap::Read, Cap::User, Cap::UserPosts],
    choices: Choices {
      search_sort: filters::SORTS,
      search_filter: filters::SEARCH_FILTERS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(5000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, user, user-posts, cities",
    guide: include_str!("../GUIDE.md"),
  };

  const BROWSER_TAB: bool = true;

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
      "ATP reads 51job as a guest, without an account",
    ))
  }

  fn logged_in(&self) -> bool {
    false
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    jobs::list(&self.api, &jobs::search_filter(query), page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    jobs::read(&self.api, post).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::company(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, _page: &PageReq) -> Result<Page<Post>> {
    users::jobs(&self.api, user).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
