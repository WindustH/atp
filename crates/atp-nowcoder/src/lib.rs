//! Nowcoder (牛客网, nowcoder.com), the job and interview community of
//! Chinese students, for all-the-platforms, through the requests the site's
//! own pages make ([`api`]): the job search of its job pages with their
//! filters, job pages ([`jobs`], [`filters`]), the 校招 schedule
//! ([`schedule`]), community posts, 面经 and the hot lists ([`posts`]) and
//! comments ([`comments`]). Reads need no account; a WeChat QR or token
//! login ([`login`]) adds saving jobs, your applications, the 内推 tab,
//! subscriptions to schedule entries, follows, likes, comments
//! ([`users`]) and unread counts ([`notifications`]).

mod api;
mod comments;
mod extra;
mod filters;
mod jobs;
mod login;
mod notifications;
mod parse;
mod posts;
mod refs;
mod schedule;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, ExperienceArgs, JobArgs, ScheduleArgs, Stage};
pub use filters::{JobKind, Nature, Salary, Size, Tab};

use std::collections::BTreeMap;
use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Comment, Ctx, Data, Error, Page, PageReq, Platform, PlatformInfo,
  PlatformKind, Post, QrStatus, QrTicket, Query, Reply, Result, User,
};

use crate::api::Api;
use crate::refs::PostRef;

/// `search --filter`: the kind of job (校招 by default), or posts.
const SEARCH_FILTERS: &[&str] = &["campus", "intern", "social", "posts"];

pub struct Nowcoder {
  api: Api,
}

impl Platform for Nowcoder {
  const INFO: PlatformInfo = PlatformInfo {
    id: "nowcoder",
    name: "Nowcoder",
    aliases: &["niuke"],
    about: "Nowcoder (牛客网): campus, internship and experienced jobs, the 校招 schedule, 面经 and the community",
    home: api::WWW,
    cookie_domains: &["nowcoder.com"],
    required_cookies: &[api::TOKEN],
    caps: &[
      Cap::QrLogin,
      Cap::TokenLogin,
      Cap::Search,
      Cap::Hot,
      Cap::Read,
      Cap::Comments,
      Cap::Replies,
      Cap::Favorites,
      Cap::Unread,
      Cap::Like,
      Cap::Favorite,
      Cap::Comment,
      Cap::Follow,
    ],
    choices: Choices {
      search_sort: posts::SORTS,
      search_filter: SEARCH_FILTERS,
      hot_category: posts::HOT_CATEGORIES,
      feed_kind: &[],
      comment_sort: comments::SORTS,
      notification_kind: &[],
    },
    min_interval: Duration::from_millis(1000),
    kind: PlatformKind::Jobs,
    token: "TOKEN",
    env: &[],
    guest: "search, jobs, read, comments, replies, hot, careers, schedule, experience",
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

  async fn login_token(&self, token: &str) -> Result<()> {
    login::token(&self.api, token)
  }

  async fn qr_start(&self) -> Result<QrTicket> {
    login::qr_start(&self.api).await
  }

  async fn qr_poll(&self, ticket: &QrTicket) -> Result<QrStatus> {
    login::qr_poll(&self.api, ticket).await
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    if query.filter.as_deref() == Some("posts") {
      return posts::search(&self.api, &query.keyword, query.sort.as_deref(), page).await;
    }
    let filter = jobs::search_filter(query)?;
    jobs::list(&self.api, &filter, page).await
  }

  async fn hot(&self, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
    posts::hot(&self.api, category, page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    match refs::post(post)? {
      PostRef::Job(id) => jobs::read(&self.api, &id).await,
      other => posts::read(&self.api, &other).await,
    }
  }

  async fn comments(
    &self,
    post: &str,
    sort: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Comment>> {
    comments::list(&self.api, post, sort, page).await
  }

  async fn replies(&self, _post: &str, comment: &str, page: &PageReq) -> Result<Page<Comment>> {
    comments::replies(&self.api, comment, page).await
  }

  async fn favorites(
    &self,
    user: Option<&str>,
    folder: Option<&str>,
    page: &PageReq,
  ) -> Result<Page<Post>> {
    if user.is_some() || folder.is_some() {
      return Err(Error::input(
        "Nowcoder shows only your own saved jobs, in one list",
      ));
    }
    jobs::favorites(&self.api, page).await
  }

  async fn unread(&self) -> Result<BTreeMap<String, u64>> {
    notifications::unread(&self.api).await
  }

  async fn like(&self, post: &str, undo: bool) -> Result<Action> {
    posts::like(&self.api, post, undo).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    if folder.is_some() {
      return Err(Error::input(
        "Nowcoder saves jobs in one list, without folders",
      ));
    }
    jobs::favorite(&self.api, post, undo).await
  }

  async fn comment(&self, post: &str, reply: &Reply) -> Result<Action> {
    comments::add(&self.api, post, reply).await
  }

  async fn follow(&self, user: &str, undo: bool) -> Result<Action> {
    users::follow(&self.api, user, undo).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
