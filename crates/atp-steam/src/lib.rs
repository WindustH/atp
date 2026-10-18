//! Steam for all-the-platforms, through the store's open JSON API
//! ([`api`]): store search, app pages with prices in a chosen storefront,
//! the front page's lists and apps' prices in several storefronts
//! ([`apps`]), and user reviews ([`reviews`]). With a Steam Web API key
//! the person makes: public profiles and the games they own ([`users`]).
//! Nothing is bought, wishlisted or written.

mod api;
mod apps;
mod extra;
mod parse;
mod refs;
mod reviews;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Cap, Choices, Comment, Ctx, Data, Page, PageReq, Platform, PlatformInfo, PlatformKind, Post,
  Query, Result, User,
};

use crate::api::Api;

pub struct Steam {
  api: Api,
}

impl Platform for Steam {
  const INFO: PlatformInfo = PlatformInfo {
    id: "steam",
    name: "Steam",
    aliases: &[],
    about: "Steam: store search, games with regional prices, top sellers and specials, user reviews; profiles and owned games with a Web API key",
    home: "https://store.steampowered.com",
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[
      Cap::TokenLogin,
      Cap::Search,
      Cap::Hot,
      Cap::Read,
      Cap::Comments,
      Cap::User,
    ],
    choices: Choices {
      hot_category: apps::HOT_CATEGORIES,
      comment_sort: reviews::SORTS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(1500),
    kind: PlatformKind::Shopping,
    token: "API_KEY|PROFILE",
    env: &[api::ENV_CC, api::ENV_LANGUAGE, api::ENV_KEY],
    guest: "search, hot, read, comments, reviews, prices",
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
    self.api.key().is_some()
  }

  async fn login_token(&self, token: &str) -> Result<()> {
    users::login(&self.api, token).await
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    apps::search(&self.api, query, page).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    apps::hot(&self.api, category, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    apps::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    reviews::list(&self.api, post, &reviews::Filter::sorted(sort), page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
