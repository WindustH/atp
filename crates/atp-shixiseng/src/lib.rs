//! Shixiseng (实习僧, shixiseng.com), the internship and campus job site,
//! for all-the-platforms, through the requests its own pages make
//! ([`api`]): the search page's job search with its filters, whose
//! internship cards are written with a private font ([`font`], [`jobs`],
//! [`filters`]), job and company pages and the company search ([`users`]);
//! with a login (WeChat mini-program code, [`login`], or a browser's
//! cookies) saved jobs and applications ([`library`]) and applying to one
//! job at a time ([`apply`]).

mod api;
mod apply;
mod extra;
mod filters;
mod font;
mod jobs;
mod library;
mod login;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, JobArgs};
pub use filters::{Degree, Kind, Months, Offer, Pay, Published, Stage, Start, Stay};

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Ctx, Data, Error, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, QrStatus, QrTicket, Query, Result, User,
};

use crate::api::Api;

pub struct Shixiseng {
  api: Api,
}

impl Platform for Shixiseng {
  const INFO: PlatformInfo = PlatformInfo {
    id: "shixiseng",
    name: "Shixiseng",
    aliases: &["sxs"],
    about: "Shixiseng (实习僧): internships and campus jobs with the site's filters, jobs, companies, saved jobs, applying to one job",
    home: api::WWW,
    cookie_domains: &["shixiseng.com"],
    required_cookies: &[api::SESSION],
    caps: &[
      Cap::QrLogin,
      Cap::Search,
      Cap::SearchUsers,
      Cap::Read,
      Cap::User,
      Cap::UserPosts,
      Cap::Favorites,
      Cap::Favorite,
    ],
    choices: Choices {
      search_sort: filters::SORTS,
      search_filter: filters::SEARCH_FILTERS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(2000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, user, user-posts",
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
    jobs::list(&self.api, &jobs::search_filter(query), page).await
  }

  async fn search_users(&self, query: &Query, page: &PageReq) -> Result<Page<User>> {
    users::search(&self.api, query, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    jobs::read(&self.api, post).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::company(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    jobs::of_company(&self.api, user, page).await
  }

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    if user.is_some() || folder.is_some() {
      return Err(Error::input(
        "Shixiseng shows only your own saved jobs, in one list",
      ));
    }
    library::favorites(&self.api, page).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    if folder.is_some() {
      return Err(Error::input(
        "Shixiseng saves jobs in one list, without folders",
      ));
    }
    library::favorite(&self.api, post, undo).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
