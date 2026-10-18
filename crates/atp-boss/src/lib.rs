//! BOSS Zhipin (BOSS直聘, zhipin.com), the job site, for
//! all-the-platforms. Its job search and job pages answer only requests
//! carrying a cookie the site's own security script computes in the
//! page, so ATP reads them through a browser tab ([`atp_core::tab`]):
//! the site's search page loads and ATP reads the answer to its own
//! request ([`jobs`], [`filters`]); job and company pages open and are
//! read as they show ([`pages`], [`users`]). The hot jobs of the site's
//! city pages answer plain requests ([`hot`]). With a QR login from the
//! BOSS app ([`login`]): your saved jobs, chatted jobs, applications and
//! interviews ([`library`]), followed companies ([`users`]), your chats
//! with recruiters ([`messages`]) and applying to one job at a time by
//! greeting its recruiter ([`apply`]), over plain requests, or made by a
//! page of the site in the tab when the site wants its cookie ([`api`]).

mod api;
mod apply;
mod extra;
mod filters;
mod font;
mod hot;
mod jobs;
mod library;
mod login;
mod messages;
mod pages;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use filters::{Degree, Experience, JobType, Pay, Size, Stage};

use std::cell::Cell;
use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Collection, Ctx, Data, Error, Page, PageReq, Platform, PlatformInfo,
  PlatformKind, Post, QrStatus, QrTicket, Query, Result, User,
};

use crate::api::Api;

pub struct Boss {
  api: Api,
  /// Whether the QR code of a login under way was scanned.
  scanned: Cell<bool>,
}

impl Platform for Boss {
  const INFO: PlatformInfo = PlatformInfo {
    id: "boss",
    name: "BOSS Zhipin",
    aliases: &["zhipin", "bosszhipin"],
    about: "BOSS Zhipin (BOSS直聘): hot jobs, job search with the site's filters, jobs, companies, saved jobs, applications, recruiter chats; read through a browser",
    home: api::WWW,
    cookie_domains: &["zhipin.com"],
    required_cookies: api::LOGIN,
    caps: &[
      Cap::QrLogin,
      Cap::Search,
      Cap::Hot,
      Cap::Read,
      Cap::User,
      Cap::UserPosts,
      Cap::Favorites,
      Cap::Favorite,
      Cap::Following,
      Cap::Follow,
      Cap::Chats,
      Cap::Messages,
    ],
    choices: Choices {
      search_filter: filters::SEARCH_FILTERS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(5000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "hot, search and jobs (the first 15 jobs), read, user, user-posts, cities, industries",
    guide: include_str!("../GUIDE.md"),
  };

  // No TAB_WINDOW: the site's check turned the browser away in a window
  // too (Chrome reports navigator.webdriver when driven over DevTools).
  const BROWSER_TAB: bool = true;

  type Extra = Command;

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self {
      api: Api::new(ctx),
      scanned: Cell::new(false),
    })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    users::whoami(&self.api).await
  }

  async fn qr_start(&self) -> Result<QrTicket> {
    login::qr_start(&self.api, &self.scanned).await
  }

  async fn qr_poll(&self, ticket: &QrTicket) -> Result<QrStatus> {
    login::qr_poll(&self.api, ticket, &self.scanned).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    hot::hot(&self.api, category, page).await
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

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    if user.is_some() || folder.is_some() {
      return Err(Error::input(
        "BOSS Zhipin shows only your own saved jobs (感兴趣), in one list",
      ));
    }
    library::favorites(&self.api, page).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    if folder.is_some() {
      return Err(Error::input(
        "BOSS Zhipin saves jobs in one list, without folders",
      ));
    }
    library::favorite(&self.api, post, undo).await
  }

  async fn following(&self, user: &str, page: &PageReq) -> Result<Page<User>> {
    users::following(&self.api, user, page).await
  }

  async fn follow(&self, user: &str, undo: bool) -> Result<Action> {
    users::follow(&self.api, user, undo).await
  }

  async fn chats(&self, parent: Option<&str>, page: &PageReq) -> Result<Page<Collection>> {
    if parent.is_some() {
      return Err(Error::unsupported(
        "chats PARENT (BOSS Zhipin chats are flat)",
      ));
    }
    messages::chats(&self.api, page).await
  }

  async fn messages(&self, chat: &str, page: &PageReq) -> Result<Page<Post>> {
    messages::messages(&self.api, chat, page).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
