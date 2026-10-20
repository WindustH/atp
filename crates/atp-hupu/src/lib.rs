//! Hupu (虎扑, bbs.hupu.com), the sports and lifestyle forum (步行街, NBA,
//! football, games, gear …), for all-the-platforms. ATP reads the forum's
//! pages as a browser loads them, with the data they were rendered from
//! ([`api`]): the hot threads of the front page and of each zone, threads
//! with their replies and 亮了 highlights ([`threads`], [`comments`]), the
//! boards and their threads ([`boards`]). With the site's cookies:
//! replying, lighting replies (亮, the site's like) and the mentions
//! ([`notifications`]).
//!
//! The site's search sits under a path its robots.txt closes, so ATP
//! offers none; profiles are shown to logged-in readers only and are not
//! read.

mod api;
mod boards;
mod comments;
mod extra;
mod notifications;
mod parse;
mod refs;
mod threads;
mod users;

pub use boards::Order;
/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Comment, Ctx, Data, Notification, Page, PageReq, Platform, PlatformInfo,
  PlatformKind, Post, Reply, Result, User,
};

use crate::api::Api;

pub struct Hupu {
  api: Api,
}

impl Platform for Hupu {
  const INFO: PlatformInfo = PlatformInfo {
    id: "hupu",
    name: "Hupu",
    aliases: &[],
    about: "Hupu (虎扑): hot threads of the forum and its zones, boards, threads with replies and 亮了 highlights; replying and lighting replies",
    home: api::BBS,
    cookie_domains: &["hupu.com"],
    required_cookies: api::LOGIN,
    caps: &[
      Cap::Hot,
      Cap::Read,
      Cap::Comments,
      Cap::Notifications,
      Cap::Comment,
    ],
    choices: Choices {
      hot_category: threads::HOT_CATEGORIES,
      comment_sort: comments::SORTS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Social,
    token: "",
    env: &[],
    guest: "hot, read, comments, boards, board",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &["boards", "board"];

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    users::whoami(&self.api).await
  }

  fn logged_in(&self) -> bool {
    self.api.logged_in()
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    threads::hot(&self.api, category, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    threads::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    comments::list(&self.api, post, sort, page).await
  }

  async fn notifications(&self, _kind: Option<&str>, page: &PageReq) -> Result<Page<Notification>> {
    notifications::mentions(&self.api, page).await
  }

  async fn comment(&self, post: &str, reply: &Reply) -> Result<Action> {
    comments::add(&self.api, post, reply).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
