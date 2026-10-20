//! SMZDM (什么值得买, smzdm.com), the deal-sharing community, for
//! all-the-platforms, read-only and without an account ([`api`]): the
//! front page's picks, the search (through a browser tab: the search site
//! answers plain requests with a script check), deals and articles read
//! from their pages ([`posts`]) and their comments ([`comments`]).
//! Nothing is bought, voted, saved, claimed or checked in.

mod api;
mod comments;
mod parse;
mod posts;
mod refs;

use std::time::Duration;

use atp_core::{
  Cap, Choices, Comment, Ctx, Data, Error, ErrorCode, NoExtra, Page, PageReq, Platform,
  PlatformInfo, PlatformKind, Post, Query, Result, User,
};

use crate::api::Api;

pub struct Smzdm {
  api: Api,
}

impl Platform for Smzdm {
  const INFO: PlatformInfo = PlatformInfo {
    id: "smzdm",
    name: "SMZDM",
    aliases: &["什么值得买"],
    about: "SMZDM (什么值得买): deals with prices, malls and 值 / 不值 votes, articles, comments, the search; read-only",
    home: api::WWW,
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[Cap::Search, Cap::Hot, Cap::Read, Cap::Comments],
    choices: Choices {
      comment_sort: comments::SORTS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Shopping,
    token: "",
    env: &[],
    guest: "search, hot, read, comments",
    guide: include_str!("../GUIDE.md"),
  };

  const BROWSER_TAB: bool = true;

  type Extra = NoExtra;

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "ATP reads SMZDM as a guest, without an account",
    ))
  }

  fn logged_in(&self) -> bool {
    false
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    posts::search(&self.api, query, page).await
  }

  async fn hot(&self, _category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    posts::hot(&self.api, page).await
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

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    match command {}
  }
}
