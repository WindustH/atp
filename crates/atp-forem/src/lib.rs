//! Forem for all-the-platforms: DEV (dev.to) by default, or another
//! Forem community through `FOREM_URL`, with its REST API ([`api`]):
//! article lists, tags, single articles with their Markdown and your
//! drafts ([`articles`]), comment threads ([`comments`]), users
//! ([`users`]), and with the API key the person makes, publishing and
//! changing articles ([`posts`]).

mod api;
mod articles;
mod comments;
mod extra;
mod parse;
mod posts;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Comment, Ctx, Data, Draft, Edit, Page, PageReq, Platform, PlatformInfo,
  PlatformKind, Post, Result, User,
};

use crate::api::Api;

pub struct Forem {
  api: Api,
}

impl Platform for Forem {
  const INFO: PlatformInfo = PlatformInfo {
    id: "forem",
    name: "Forem",
    aliases: &["devto"],
    about: "Forem communities (DEV / dev.to by default): articles, tags, comment threads, users; publishing and editing articles with an API key",
    home: "https://dev.to",
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[
      Cap::TokenLogin,
      Cap::Hot,
      Cap::Feed,
      Cap::Read,
      Cap::Comments,
      Cap::Replies,
      Cap::User,
      Cap::UserPosts,
      Cap::Publish,
      Cap::PostArticle,
      Cap::PostVisibility,
      Cap::Edit,
      Cap::EditTitle,
      Cap::EditTopics,
      Cap::EditVisibility,
    ],
    choices: Choices {
      hot_category: articles::HOT_CATEGORIES,
      feed_kind: articles::FEED_KINDS,
      ..Choices::NONE
    },
    min_interval: Duration::from_secs(1),
    kind: PlatformKind::Developer,
    token: "API_KEY",
    env: &[api::ENV_INSTANCE, api::ENV_KEY],
    guest: "hot, feed, read, comments, replies, user, user-posts, tag, tags",
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
    users::login(&self.api, token)
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    articles::hot(&self.api, category, page).await
  }

  async fn feed(&self, kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    articles::feed(&self.api, kind, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    articles::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    _sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    comments::comments(&self.api, post, page).await
  }

  async fn replies(&self, _post: &str, comment: &str, page: &PageReq) -> Result<Page<Comment>> {
    comments::replies(&self.api, comment, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    articles::by_user(&self.api, user, page).await
  }

  async fn publish(&self, draft: &Draft) -> Result<Action> {
    posts::publish(&self.api, draft).await
  }

  async fn edit_post(&self, post: &str, edit: &Edit) -> Result<Action> {
    posts::edit(&self.api, post, edit).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
