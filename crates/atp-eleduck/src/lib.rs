//! Eleduck (电鸭社区, eleduck.com), the remote-work community, for
//! all-the-platforms, through the JSON API the site's own pages call
//! ([`api`]): the jobs channel and the discussion categories, search,
//! posts with what they say about the job ([`posts`], [`parse`]), comments
//! with their replies ([`comments`]), users ([`users`]), categories and
//! their tags ([`categories`]). Reads need no account; a WeChat QR or
//! token login ([`login`]) adds upvotes, saving, comments, your saved and
//! viewed posts and job applications ([`library`]) and messages
//! ([`notifications`]).

mod api;
mod categories;
mod comments;
mod extra;
mod library;
mod login;
mod notifications;
mod parse;
mod posts;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{ApplicationStatus, Command, JobArgs, JobKind};

use std::collections::BTreeMap;
use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Comment, Ctx, Data, Error, Notification, Page, PageReq, Platform,
  PlatformInfo, PlatformKind, Post, QrStatus, QrTicket, Query, Reply, Result, User,
};

use crate::api::Api;

pub struct Eleduck {
  api: Api,
}

impl Platform for Eleduck {
  const INFO: PlatformInfo = PlatformInfo {
    id: "eleduck",
    name: "Eleduck",
    aliases: &[],
    about: "Eleduck (电鸭社区): remote and part-time jobs, discussions, comments, your applications",
    home: api::WEB,
    cookie_domains: &["eleduck.com"],
    required_cookies: &[api::TOKEN],
    caps: &[
      Cap::QrLogin,
      Cap::TokenLogin,
      Cap::Search,
      Cap::Hot,
      Cap::Feed,
      Cap::Read,
      Cap::Comments,
      Cap::User,
      Cap::UserPosts,
      Cap::Favorites,
      Cap::History,
      Cap::Notifications,
      Cap::Unread,
      Cap::Like,
      Cap::Favorite,
      Cap::Comment,
    ],
    choices: Choices {
      search_sort: posts::SEARCH_SORTS,
      search_filter: posts::SEARCH_FILTERS,
      hot_category: posts::HOT_CATEGORIES,
      feed_kind: &[],
      comment_sort: comments::SORTS,
      notification_kind: notifications::KINDS,
    },
    min_interval: Duration::from_millis(1000),
    kind: PlatformKind::Jobs,
    token: "TOKEN",
    env: &[],
    guest: "search, hot, read, comments, user, user-posts, jobs, categories, tags",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &["jobs", "categories", "tags", "applications"];

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    users::whoami(&self.api).await
  }

  async fn login_token(&self, token: &str) -> Result<()> {
    login::token(&self.api, token)
  }

  async fn qr_start(&self) -> Result<QrTicket> {
    login::qr_start(&self.api).await
  }

  async fn qr_poll(&self, ticket: &QrTicket) -> Result<QrStatus> {
    login::qr_poll(&self.api, ticket).await
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    posts::search(&self.api, query, page).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    posts::hot(&self.api, category, page).await
  }

  async fn feed(&self, _kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    posts::followed(&self.api, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    posts::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    comments::list(&self.api, post, sort, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    posts::user_posts(&self.api, user, page).await
  }

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    if user.is_some() || folder.is_some() {
      return Err(Error::input(
        "Eleduck shows only your own saved posts, in one list",
      ));
    }
    library::favorites(&self.api, page).await
  }

  async fn history(&self, page: &PageReq) -> Result<Page<Post>> {
    library::history(&self.api, page).await
  }

  async fn notifications(&self, kind: Option<&str>, page: &PageReq) -> Result<Page<Notification>> {
    notifications::list(&self.api, kind, page).await
  }

  async fn unread(&self) -> Result<BTreeMap<String, u64>> {
    notifications::unread(&self.api).await
  }

  async fn like(&self, post: &str, undo: bool) -> Result<Action> {
    posts::like(&self.api, post, undo).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    if folder.is_some() {
      return Err(Error::input(
        "Eleduck saves posts in one list, without folders",
      ));
    }
    posts::favorite(&self.api, post, undo).await
  }

  async fn comment(&self, post: &str, reply: &Reply) -> Result<Action> {
    comments::add(&self.api, post, reply).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
