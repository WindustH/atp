//! NCSS (国家大学生就业服务平台, www.ncss.cn), the national job board of
//! China's Ministry of Education for college graduates, for
//! all-the-platforms, through the pages of its student side and the JSON
//! their scripts load ([`api`]): the job search with its filters ([`jobs`],
//! [`filters`]), job and company pages ([`parse`], [`users`]) and job
//! fairs ([`fairs`]). Reads need no account; a 学信网 (CHSI) session taken
//! from a browser adds saving jobs and applying to one at a time
//! ([`apply`]).

mod api;
mod apply;
mod codes;
mod extra;
mod fairs;
mod filters;
mod jobs;
mod parse;
mod refs;
mod users;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::{Command, FairArgs, Filters, JobArgs};
pub use fairs::FairList;
pub use filters::{Degree, JobType, Kind, Property, Salary, Source};

use std::time::Duration;

use atp_core::{
  Action, Cap, Choices, Ctx, Data, Error, Page, PageReq, Platform, PlatformInfo, PlatformKind,
  Post, Query, Result, User,
};

use crate::api::Api;

pub struct Ncss {
  api: Api,
}

impl Platform for Ncss {
  const INFO: PlatformInfo = PlatformInfo {
    id: "ncss",
    name: "NCSS",
    aliases: &[],
    about: "NCSS (国家大学生就业服务平台): the national job board for graduates, its job fairs and companies",
    home: api::WWW,
    cookie_domains: &["ncss.cn"],
    required_cookies: &[api::SESSION],
    caps: &[
      Cap::Search,
      Cap::Read,
      Cap::User,
      Cap::UserPosts,
      Cap::Favorite,
    ],
    choices: Choices {
      search_filter: filters::SEARCH_FILTERS,
      ..Choices::NONE
    },
    min_interval: Duration::from_millis(2000),
    kind: PlatformKind::Jobs,
    token: "",
    env: &[],
    guest: "search, jobs, read, user, user-posts, fairs, fair, fair-companies, areas, categories, industries",
    guide: include_str!("../GUIDE.md"),
  };

  type Extra = Command;
  const EXTRA_READS: &'static [&'static str] = &[
    "jobs",
    "areas",
    "categories",
    "industries",
    "fairs",
    "fair",
    "fair-companies",
  ];

  fn new(ctx: Ctx) -> Result<Self> {
    Ok(Self { api: Api::new(ctx) })
  }

  fn ctx(&self) -> &Ctx {
    &self.api.ctx
  }

  async fn whoami(&self) -> Result<User> {
    users::whoami(&self.api).await
  }

  async fn search(&self, query: &Query, page: &PageReq) -> Result<Page<Post>> {
    jobs::list(&self.api, &jobs::search_filter(query), page).await
  }

  async fn read(&self, post: &str) -> Result<Post> {
    jobs::read(&self.api, post).await
  }

  async fn user(&self, user: &str) -> Result<User> {
    users::company(&self.api, user).await
  }

  async fn user_posts(&self, user: &str, page: &PageReq) -> Result<Page<Post>> {
    jobs::company(&self.api, user, page).await
  }

  async fn favorite(&self, post: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
    if folder.is_some() {
      return Err(Error::input("NCSS saves jobs in one list, without folders"));
    }
    if undo {
      return Err(Error::unsupported(
        "unfavorite (the site takes back a saved job by an id of the saved entry, which ATP cannot read; do it on ncss.cn)",
      ));
    }
    apply::favorite(&self.api, post).await
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
