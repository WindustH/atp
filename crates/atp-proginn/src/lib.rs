//! Proginn (程序员客栈, proginn.com), where developers find remote and
//! on-site freelance work, for all-the-platforms: the job board of
//! job.proginn.com through the API its own pages call ([`api`]): the
//! search with its filters ([`jobs`], [`filters`]) and job pages
//! ([`parse`]). With the cookies of a logged-in browser: the account
//! ([`users`]), its unread counts ([`notifications`]) and applying to one
//! job at a time ([`apply`]). Developers' profiles are left alone, and
//! whoever posted a job is left out of everything ATP prints.

mod api;
mod apply;
mod extra;
mod filters;
mod jobs;
mod notifications;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use filters::WorkType;

use std::collections::BTreeMap;
use std::time::Duration;

use atp_core::{
  Cap, Choices, Ctx, Data, Page, PageReq, Platform, PlatformInfo, PlatformKind, Post, Query,
  Result, User,
};

use crate::api::Api;

pub struct Proginn {
  api: Api,
}

impl Platform for Proginn {
  const INFO: PlatformInfo = PlatformInfo {
    id: "proginn",
    name: "Proginn",
    aliases: &["kezhan"],
    about: "Proginn (程序员客栈): remote and on-site developer jobs, filters by city and role, applying to a job",
    home: api::JOB,
    cookie_domains: &["proginn.com"],
    required_cookies: &[api::TOKEN],
    caps: &[Cap::Search, Cap::Hot, Cap::Read, Cap::Unread],
    choices: Choices {
      search_filter: filters::WORK_TYPES,
      hot_category: filters::WORK_TYPES,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(2500),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, hot, jobs, read, cities, roles",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &["jobs", "cities", "roles"];

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
    jobs::search(&self.api, query, page).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    jobs::hot(&self.api, category, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    jobs::read(&self.api, post).await
  }

  async fn unread(&self) -> Result<BTreeMap<String, u64>> {
    notifications::unread(&self.api).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
