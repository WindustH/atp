//! 知识星球 (zsxq, Knowledge Planet), the paid communities (星球) of
//! creators, for all-the-platforms. ATP reads the JSON API the web app
//! (wx.zsxq.com) calls, signed as the app signs it ([`api`], [`sign`]):
//! the groups you joined ([`groups`]), their topics with pictures, files,
//! articles and Q&A answers, their digests and the feed of all of them
//! ([`topics`]), comments ([`comments`]), search ([`search`]), members
//! ([`users`]) and the files of a topic ([`files`]). With the login:
//! liking, commenting and posting text ([`interactions`]).
//!
//! Nearly all of it needs a login and the membership of the group; what
//! answers without one is a group's public preview and a member's public
//! profile.

mod api;
mod comments;
mod extra;
mod files;
mod groups;
mod interactions;
mod parse;
mod refs;
mod search;
mod sign;
mod topics;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;
pub use topics::Scope;

use std::path::{Path, PathBuf};
use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Collection, Comment, Ctx, Data, Draft, Error, Media, Page, PageReq,
  Platform, PlatformInfo, PlatformKind, Post, Query, Reply, Result, User,
};

use crate::api::Api;

pub struct Zsxq {
  api: Api,
}

impl Platform for Zsxq {
  const INFO: PlatformInfo = PlatformInfo {
    id: "zsxq",
    name: "ZSXQ",
    aliases: &["知识星球"],
    about: "ZSXQ (知识星球): the groups you joined, their topics, digests, Q&A, comments, files and search; liking, commenting and posting",
    home: api::WEB,
    cookie_domains: &["zsxq.com"],
    required_cookies: api::LOGIN,
    caps: &[
      Cap::Search,
      Cap::Hot,
      Cap::Feed,
      Cap::Read,
      Cap::Comments,
      Cap::User,
      Cap::UserPosts,
      Cap::Collections,
      Cap::Like,
      Cap::Comment,
      Cap::Publish,
      Cap::Download,
    ],
    choices: Choices::NONE,
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Social,
    token: "",
    env: &[],
    guest: "group, topics (a group's public preview), user",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &["groups", "group", "topics", "group-search"];

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
    search::all(&self.api, &query.keyword, page).await
  }

  async fn hot(&self, _category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    topics::hot(&self.api, page).await
  }

  async fn feed(&self, _kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    topics::feed(&self.api, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    topics::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    _sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    comments::list(&self.api, post, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    users::posts(&self.api, user, page).await
  }

  async fn collections(&self, user: Option<&str>, _page: &PageReq) -> Result<Page<Collection>> {
    if user.is_some() {
      return Err(
        Error::unsupported("collections USER").with_hint("only the groups you joined are listed"),
      );
    }
    Ok(Page::last(groups::joined(&self.api).await?))
  }

  async fn like(&self, post: &str, undo: bool) -> Result<Action> {
    interactions::like(&self.api, post, undo).await
  }

  async fn comment(&self, post: &str, reply: &Reply) -> Result<Action> {
    interactions::comment(&self.api, post, reply).await
  }

  async fn publish(&self, draft: &Draft) -> Result<Action> {
    interactions::publish(&self.api, draft).await
  }

  async fn media(&self, post: &str, _audio_only: bool) -> Result<(Post, Vec<Media>)> {
    files::media(&self.api, post).await
  }

  async fn save_media(&self, media: &Media, dir: &Path, _name: &str) -> Option<Result<PathBuf>> {
    files::save(&self.api, media, dir).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
