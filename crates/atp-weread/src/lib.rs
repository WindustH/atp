//! WeRead (微信读书) for all-the-platforms, through the web reader's own
//! routes on weread.qq.com ([`api`]): store search, rankings and
//! categories without an account; with a login, books, public reviews,
//! your shelf, reading progress, and your highlights and thoughts, which
//! export as Markdown. With an API key (`wrk-…`) of WeRead's official
//! Agent Gateway ([`gateway`]), those reads go through it instead, and
//! recommendations, similar books and reading statistics join them. Book
//! contents are never fetched: only tables of contents, WeRead's AI
//! outline and the passages around a searched word ([`chapters`]).

mod api;
mod books;
mod chapters;
mod discover;
mod extra;
mod gateway;
mod ids;
mod login;
mod notes;
mod parse;
mod reading;
mod refs;
mod shelf;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Cap, Choices, Comment, Ctx, Data, Page, PageReq, Platform, PlatformInfo, PlatformKind, Post,
  QrStatus, QrTicket, Query, Result, User, credentials,
};

use crate::api::Api;

pub struct WeRead {
  api: Api,
}

impl Platform for WeRead {
  const INFO: PlatformInfo = PlatformInfo {
    id: "weread",
    name: "WeRead",
    aliases: &["wr"],
    about: "WeRead (微信读书): store, rankings, your shelf, progress, highlights, thoughts and reading statistics",
    home: "https://weread.qq.com",
    cookie_domains: &["weread.qq.com"],
    required_cookies: api::LOGIN_COOKIES,
    caps: &[
      Cap::QrLogin,
      Cap::TokenLogin,
      Cap::Search,
      Cap::Hot,
      Cap::Feed,
      Cap::Read,
      Cap::Comments,
      Cap::User,
      Cap::Favorites,
    ],
    choices: Choices {
      hot_category: books::RANKING_NAMES,
      search_filter: books::SCOPE_NAMES,
      comment_sort: books::REVIEW_SORTS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(800),
    kind: PlatformKind::Social,
    token: "wrk-…",
    env: &[gateway::KEY_ENV],
    guest: "search, hot, category, book-search",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  /// A cookie session, or an API key.
  fn logged_in(&self) -> bool {
    self.api.logged_in()
  }

  async fn whoami(&self) -> Result<User> {
    users::whoami(&self.api).await
  }

  async fn login_token(&self, token: &str) -> Result<()> {
    let key = gateway::check_key(token)?;
    credentials::save(&self.api.ctx, &[(gateway::API_KEY, key.as_str())]);
    Ok(())
  }

  async fn token_whoami(&self) -> Result<User> {
    users::key_owner(&self.api).await
  }

  async fn qr_start(&self) -> Result<QrTicket> {
    login::qr_start(&self.api).await
  }

  async fn qr_poll(&self, ticket: &QrTicket) -> Result<QrStatus> {
    login::qr_poll(&self.api, ticket).await
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    books::search(&self.api, query, page).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    books::hot(&self.api, category, page).await
  }

  async fn feed(&self, _kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    discover::recommended(&self.api, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    books::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    books::reviews(&self.api, post, sort, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    shelf::shelf(&self.api, user, folder, page).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
