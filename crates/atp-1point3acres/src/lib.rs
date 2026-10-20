//! 1Point3Acres (一亩三分地, 1point3acres.com), the forum of Chinese
//! students and workers abroad (面经, 内推, offers, visas, life), for
//! all-the-platforms, through the JSON API the site's own pages call
//! ([`api`]): the hot and newest threads and a board's threads
//! ([`threads`]), the boards ([`boards`]), threads read whole and their
//! replies ([`comments`]). With the site's cookies: the search, profiles
//! and a user's threads ([`users`]), every answer to a reply, and replying
//! to a thread.
//!
//! Parts of threads the site keeps from readers without enough points
//! (`[hide]`) or without a login stay hidden: ATP shows what the session
//! may see and says so. Check-ins and the daily question (behind a
//! Turnstile captcha) are not offered.

mod api;
mod boards;
mod comments;
mod extra;
mod parse;
mod refs;
mod threads;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;
pub use threads::Order;

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Comment, Ctx, Data, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Query, Reply, Result, User,
};

use crate::api::Api;

pub struct OnePoint3Acres {
  api: Api,
}

impl Platform for OnePoint3Acres {
  const INFO: PlatformInfo = PlatformInfo {
    id: "1point3acres",
    name: "1Point3Acres",
    aliases: &["1p3a", "yimu"],
    about: "1Point3Acres (一亩三分地): 面经, 内推 and life abroad; hot and newest threads, boards, threads with their replies",
    home: api::WWW,
    cookie_domains: &["1point3acres.com"],
    required_cookies: &[],
    caps: &[
      Cap::Search,
      Cap::Hot,
      Cap::Feed,
      Cap::Read,
      Cap::Comments,
      Cap::Replies,
      Cap::User,
      Cap::UserPosts,
      Cap::Comment,
    ],
    choices: Choices {
      search_sort: &[],
      search_filter: &[],
      hot_category: &[],
      feed_kind: &[],
      comment_sort: comments::SORTS,
      notification_kind: &[],
    },
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "hot, feed, read, comments, boards, board",
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

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    threads::search(&self.api, query, page).await
  }

  async fn hot(&self, _category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    threads::hot(&self.api, page).await
  }

  async fn feed(&self, _kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    threads::newest(&self.api, page).await
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

  async fn replies(&self, _post: &str, comment: &str, page: &PageReq) -> Result<Page<Comment>> {
    comments::replies(&self.api, comment, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    threads::by_user(&self.api, user, page).await
  }

  async fn comment(&self, post: &str, reply: &Reply) -> Result<Action> {
    comments::add(&self.api, post, reply).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
