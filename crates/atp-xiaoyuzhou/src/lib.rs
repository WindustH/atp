//! Xiaoyuzhou (小宇宙) for all-the-platforms. The public pages of
//! www.xiaoyuzhoufm.com ([`api`]) give shows and the episodes their pages
//! list ([`shows`]), episodes with show notes, hot comments and their
//! audio files ([`episodes`]), and users with the shows they host
//! ([`users`]), without an account. A login to the app's API ([`app`],
//! [`login`]: a texted code or tokens) adds transcripts
//! ([`transcripts`]), the play history with progress ([`history`]) and
//! every episode of a show.

mod api;
mod app;
mod episodes;
mod extra;
mod history;
mod login;
mod parse;
mod refs;
mod shows;
mod transcripts;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Cap, Choices, Comment, Ctx, Data, Media, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Result, User,
};

use crate::api::Api;

pub struct Xiaoyuzhou {
  api: Api,
}

impl Platform for Xiaoyuzhou {
  const INFO: PlatformInfo = PlatformInfo {
    id: "xiaoyuzhou",
    name: "Xiaoyuzhou",
    aliases: &["xyz"],
    about: "Xiaoyuzhou (小宇宙): podcast shows, episodes with show notes, hot comments, downloads, transcripts and play history",
    home: api::BASE,
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[
      Cap::TokenLogin,
      Cap::Read,
      Cap::Comments,
      Cap::User,
      Cap::UserPosts,
      Cap::History,
      Cap::Download,
    ],
    choices: Choices::NONE,
    min_interval: Duration::from_millis(1500),
    kind: PlatformKind::Music,
    token: "ACCESS_TOKEN|REFRESH_TOKEN",
    env: &[],
    guest: "read, comments, user, user-posts (the newest 15 episodes), download",
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
    self.api.app().logged_in()
  }

  async fn login_token(&self, token: &str) -> Result<()> {
    login::token(&self.api, token).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    episodes::read_any(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    _sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    episodes::comments(&self.api, post, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    users::posts(&self.api, user, page).await
  }

  async fn history(&self, page: &PageReq) -> Result<Page<Post>> {
    history::history(&self.api, page).await
  }

  async fn media(&self, post: &str, _audio_only: bool) -> Result<(Post, Vec<Media>)> {
    episodes::media(&self.api, post).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
