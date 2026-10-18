//! Zhilian Zhaopin (智联招聘, zhaopin.com), the job site, for
//! all-the-platforms, through the requests its own pages make ([`api`]):
//! the job search of www with the site's filters ([`jobs`], [`filters`]),
//! job and company pages of the mobile site ([`parse`], [`users`]); with
//! the login's cookies from a browser, the account and applying to one job
//! at a time ([`apply`]).

mod api;
mod apply;
mod extra;
mod filters;
mod jobs;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use filters::{Degree, Experience, JobType};

use std::time::Duration;

use atp_core::{
  Cap, Choices, Ctx, Data, Page, PageReq, Platform, PlatformInfo, PlatformKind, Post, Query,
  Result, User,
};

use crate::api::Api;

pub struct Zhilian {
  api: Api,
}

impl Platform for Zhilian {
  const INFO: PlatformInfo = PlatformInfo {
    id: "zhilian",
    name: "Zhilian",
    aliases: &["zhaopin"],
    about: "Zhilian Zhaopin (智联招聘): job search with the site's filters, jobs, companies, applying to one job",
    home: api::WWW,
    cookie_domains: &["zhaopin.com"],
    required_cookies: &[api::AT, api::RT],
    caps: &[Cap::Search, Cap::Read, Cap::User],
    choices: Choices {
      search_filter: filters::SEARCH_FILTERS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, user, cities",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    users::whoami(&self.api).await
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

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
