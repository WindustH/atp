//! LinkedIn's jobs for all-the-platforms, read as a guest, the way its
//! public jobs pages are: the job search by words, place, date posted and
//! company ([`jobs`], [`filters`]), job pages, and companies with their
//! open jobs ([`users`]), all through the requests those pages make
//! ([`api`]) and parsed from the HTML they answer with ([`parse`]).
//!
//! No account is used. Nothing is applied to, saved, followed or sent, and
//! people (recruiters, the hiring team, employees) are left out of
//! everything ATP prints, `raw` included.

mod api;
mod extra;
mod filters;
mod jobs;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use filters::Posted;

use std::time::Duration;

use atp_core::{
  Cap, Choices, Ctx, Data, Error, ErrorCode, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Query, Result, User,
};

use crate::api::Api;

pub struct LinkedIn {
  api: Api,
}

impl Platform for LinkedIn {
  const INFO: PlatformInfo = PlatformInfo {
    id: "linkedin",
    name: "LinkedIn",
    aliases: &[],
    about: "LinkedIn jobs: the job search by place, date posted and company, job pages, companies and their open jobs, without an account",
    home: api::WWW,
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[Cap::Search, Cap::Read, Cap::User, Cap::UserPosts],
    choices: Choices::NONE,
    min_interval: Duration::from_millis(4000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, user, user-posts",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &["jobs"];

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "ATP reads LinkedIn's jobs as a guest, without an account",
    ))
  }

  fn logged_in(&self) -> bool {
    false
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    jobs::search(&self.api, query, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    jobs::read(&self.api, post).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    jobs::company(&self.api, user, page).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
