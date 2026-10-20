//! Pixiv (pixiv.net), the Japanese site of illustrations, manga and
//! novels, for all-the-platforms. ATP talks to the JSON API the site's
//! own pages call ([`api`]): artworks with every page and novels with
//! their text ([`works`]), the search, the rankings ([`rankings`]),
//! artists with their works and the artists they follow ([`users`]),
//! comments ([`comments`]) and original-size downloads. With the site's
//! cookies: your bookmarks and their tags, the new works of the artists
//! you follow ([`library`]), likes, bookmarks and follows
//! ([`interactions`]).
//!
//! Works for adults (R-18, R-18G) are never asked for or shown without a
//! login; with one, the site applies the account's own setting.

mod api;
mod comments;
mod extra;
mod interactions;
mod library;
mod parse;
mod rankings;
mod refs;
mod users;
mod works;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Collection, Comment, Ctx, Data, Media, Page, PageReq, Platform,
  PlatformInfo, PlatformKind, Post, Query, Result, User,
};

use crate::api::Api;

pub struct Pixiv {
  api: Api,
}

impl Platform for Pixiv {
  const INFO: PlatformInfo = PlatformInfo {
    id: "pixiv",
    name: "Pixiv",
    aliases: &[],
    about: "Pixiv: illustrations, manga, ugoira and novels, search, rankings, artists, comments, original-size downloads; bookmarks, likes and follows with a login",
    home: "https://www.pixiv.net/",
    cookie_domains: &["pixiv.net"],
    required_cookies: api::LOGIN,
    caps: &[
      Cap::Search,
      Cap::Hot,
      Cap::Feed,
      Cap::Read,
      Cap::Comments,
      Cap::Replies,
      Cap::User,
      Cap::UserPosts,
      Cap::Following,
      Cap::Collections,
      Cap::Favorites,
      Cap::Like,
      Cap::Favorite,
      Cap::Follow,
      Cap::Download,
    ],
    choices: Choices {
      search_sort: works::SORTS,
      search_filter: works::FILTERS,
      hot_category: rankings::MODES,
      feed_kind: library::FEED_KINDS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(3000),
    kind: PlatformKind::Social,
    token: "",
    env: &[],
    guest: "search, hot, read, comments, replies, user, user-posts, novels, download (works for all ages only)",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &["novels"];

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
    works::search(&self.api, query, page).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    rankings::hot(&self.api, category, page).await
  }

  async fn feed(&self, kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    library::feed(&self.api, kind, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    works::read(&self.api, post).await
  }

  async fn comments(
    &self,
    post: &str,
    _sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    comments::list(&self.api, post, page).await
  }

  async fn replies(&self, post: &str, comment: &str, page: &PageReq) -> Result<Page<Comment>> {
    comments::replies(&self.api, post, comment, page).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::user(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    users::artworks(&self.api, user, page).await
  }

  async fn following(&self, user: &str, page: &PageReq) -> Result<Page<User>> {
    users::following(&self.api, user, page).await
  }

  async fn collections(&self, user: Option<&str>, page: &PageReq) -> Result<Page<Collection>> {
    library::collections(&self.api, user, page).await
  }

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    library::favorites(&self.api, user, folder, page).await
  }

  async fn like(&self, post: &str, undo: bool) -> Result<Action> {
    interactions::like(&self.api, post, undo).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    interactions::favorite(&self.api, post, folder, undo).await
  }

  async fn follow(&self, user: &str, undo: bool) -> Result<Action> {
    interactions::follow(&self.api, user, undo).await
  }

  async fn media(&self, post: &str, _audio_only: bool) -> Result<(Post, Vec<Media>)> {
    works::media(&self.api, post).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
