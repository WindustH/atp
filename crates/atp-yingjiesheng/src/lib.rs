//! Yingjiesheng (应届生求职网, yingjiesheng.com), the campus job site of
//! 51job, for all-the-platforms: its campus recruitment talks (宣讲会),
//! read from the site's older pages in GBK ([`api`], [`talks`]) with the
//! codes of their filters ([`filters`]), and their times from the mobile
//! site ([`parse`]). Everything is read without an account. Its job
//! search, which signs each request, is not offered.

mod api;
mod extra;
mod filters;
mod parse;
mod refs;
mod talks;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, TalkArgs};

use std::time::Duration;

use atp_core::{
  Cap, Choices, Ctx, Data, Error, ErrorCode, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Query, Result, User,
};

use crate::api::Api;
use crate::talks::Filter;

pub struct Yingjiesheng {
  api: Api,
}

impl Platform for Yingjiesheng {
  const INFO: PlatformInfo = PlatformInfo {
    id: "yingjiesheng",
    name: "Yingjiesheng",
    aliases: &["yjs"],
    about: "Yingjiesheng (应届生求职网): campus recruitment talks (宣讲会) by province, city, school, company, day and industry",
    home: api::WWW,
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[Cap::Search, Cap::Hot, Cap::Read, Cap::UserPosts],
    choices: Choices::NONE,
    min_interval: Duration::from_millis(2000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, hot, read, user-posts, talks, provinces, industries, schools",
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
    Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "ATP reads Yingjiesheng without an account; there is nothing to log in to",
    ))
  }

  fn logged_in(&self) -> bool {
    false
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    let filter = Filter {
      words: query.keyword.clone(),
      ..Filter::default()
    };
    talks::list(&self.api, &filter, page).await
  }

  async fn hot(&self, _category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    talks::list(&self.api, &Filter::default(), page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    talks::read(&self.api, post).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    let filter = Filter {
      company: Some(refs::company(user)?),
      ..Filter::default()
    };
    talks::list(&self.api, &filter, page).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
