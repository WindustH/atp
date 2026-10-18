//! Liepin (猎聘, liepin.com), the job site, for all-the-platforms: the
//! job search of its 职位 page with the site's filters ([`jobs`],
//! [`filters`]), job and company pages ([`parse`]), companies' jobs and
//! following companies ([`users`]); with a QR login from the Liepin app
//! ([`login`]) or the browser's cookies, your saved jobs, applications
//! and invitations ([`library`]), applying to one job at a time
//! ([`apply`]) and your chats with recruiters ([`messages`]). All of it
//! through the requests the site's own pages make ([`api`]).

mod api;
mod apply;
mod extra;
mod filters;
mod jobs;
mod library;
mod login;
mod messages;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{ApplicationStatus, Command, JobArgs};
pub use filters::{Degree, Experience, JobKind, Salary, Size, Stage};

use std::collections::BTreeMap;
use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Collection, Ctx, Data, Error, Message, Page, PageReq, Platform,
  PlatformInfo, PlatformKind, Post, QrStatus, QrTicket, Query, Result, User,
};

use crate::api::Api;

pub struct Liepin {
  api: Api,
}

impl Platform for Liepin {
  const INFO: PlatformInfo = PlatformInfo {
    id: "liepin",
    name: "Liepin",
    aliases: &[],
    about: "Liepin (猎聘): job search with the site's filters, jobs, companies, saved jobs, applications, recruiter chats",
    home: api::WWW,
    cookie_domains: &["liepin.com"],
    required_cookies: &[api::USER],
    caps: &[
      Cap::QrLogin,
      Cap::Search,
      Cap::Read,
      Cap::User,
      Cap::UserPosts,
      Cap::Favorites,
      Cap::Favorite,
      Cap::Follow,
      Cap::Chats,
      Cap::Messages,
      Cap::Send,
      Cap::Unread,
    ],
    choices: Choices {
      search_filter: filters::SEARCH_FILTERS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, user, user-posts, cities, industries",
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

  async fn qr_start(&self) -> Result<QrTicket> {
    login::qr_start(&self.api).await
  }

  async fn qr_poll(&self, ticket: &QrTicket) -> Result<QrStatus> {
    login::qr_poll(&self.api, ticket).await
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

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    if user.is_some() || folder.is_some() {
      return Err(Error::input(
        "Liepin shows only your own saved jobs, in one list",
      ));
    }
    library::favorites(&self.api, page).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    if folder.is_some() {
      return Err(Error::input(
        "Liepin saves jobs in one list, without folders",
      ));
    }
    library::favorite(&self.api, post, undo).await
  }

  async fn follow(&self, user: &str, undo: bool) -> Result<Action> {
    users::follow(&self.api, user, undo).await
  }

  async fn chats(&self, parent: Option<&str>, page: &PageReq) -> Result<Page<Collection>> {
    if parent.is_some() {
      return Err(Error::unsupported("chats PARENT (Liepin chats are flat)"));
    }
    messages::chats(&self.api, page).await
  }

  async fn messages(&self, chat: &str, page: &PageReq) -> Result<Page<Post>> {
    messages::messages(&self.api, chat, page).await
  }

  async fn send(&self, chat: &str, message: &Message) -> Result<Action> {
    messages::send(&self.api, chat, message).await
  }

  async fn unread(&self) -> Result<BTreeMap<String, u64>> {
    messages::unread(&self.api).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
