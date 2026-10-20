//! Pinterest for all-the-platforms. ATP talks to the resource API the
//! site's own pages call ([`api`]): pins with their original picture or
//! video, the search of pins, a user's pins and a board's ([`pins`]),
//! boards ([`boards`]), people ([`users`]); with the site's cookies,
//! saving pins, creating pins and boards ([`interactions`]).

mod api;
mod boards;
mod extra;
mod interactions;
mod listed;
mod parse;
mod pins;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Collection, Ctx, Data, FolderSpec, Page, PageReq, Platform, PlatformInfo,
  PlatformKind, Post, Query, Result, User,
};

use crate::api::Api;

pub struct Pinterest {
  api: Api,
}

impl Platform for Pinterest {
  const INFO: PlatformInfo = PlatformInfo {
    id: "pinterest",
    name: "Pinterest",
    aliases: &["pin"],
    about: "Pinterest: pins with their original picture or video, the search of pins, boards and people, profiles, boards; saving pins, creating pins and boards with a login",
    home: "https://www.pinterest.com",
    cookie_domains: &["pinterest.com"],
    required_cookies: api::LOGIN,
    caps: &[
      Cap::Search,
      Cap::SearchUsers,
      Cap::SearchTopics,
      Cap::Read,
      Cap::User,
      Cap::UserPosts,
      Cap::Collections,
      Cap::Favorite,
      Cap::FolderCreate,
      Cap::FolderDescription,
      Cap::FolderPrivacy,
      Cap::Download,
    ],
    choices: Choices::NONE,
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Social,
    token: "",
    env: &[],
    guest: "search (pins, boards, people), read, user, user-posts, collections USER, board, download",
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

  fn logged_in(&self) -> bool {
    self.api.logged_in()
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    pins::search(&self.api, query, page).await
  }

  async fn search_users(&self, query: &Query, page: &PageReq) -> Result<Page<User>> {
    users::search(&self.api, query, page).await
  }

  async fn search_topics(&self, query: &Query, page: &PageReq) -> Result<Page<Collection>> {
    boards::search(&self.api, query, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    pins::read(&self.api, post).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    pins::by_user(&self.api, user, page).await
  }

  async fn collections(&self, user: Option<&str>, page: &PageReq) -> Result<Page<Collection>> {
    boards::list(&self.api, user, page).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    interactions::save(&self.api, post, folder, undo).await
  }

  async fn folder_create(&self, folder: &FolderSpec) -> Result<Action> {
    boards::create(&self.api, folder).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
