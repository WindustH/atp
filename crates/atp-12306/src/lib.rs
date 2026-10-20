//! 12306 (China Railway 中国铁路) for all-the-platforms, read-only and
//! without an account, through the JSON its query page calls ([`api`]):
//! the stations ([`stations`]), the trains between two stations on a day
//! with the seats left in each class, a train's stops and its fares
//! ([`trains`]). Nothing is booked, ordered or waitlisted; there is no
//! login.

mod api;
mod extra;
mod parse;
mod refs;
mod stations;
mod trains;

/// The commands only this platform has, for [`Platform::run_extra`](atp_core::Platform::run_extra).
pub use extra::Command;

use std::time::Duration;

use atp_core::{
  Choices, Ctx, Data, Error, ErrorCode, Platform, PlatformInfo, PlatformKind, Result, User,
};

use crate::api::Api;

pub struct Railway12306 {
  api: Api,
}

impl Platform for Railway12306 {
  const INFO: PlatformInfo = PlatformInfo {
    id: "12306",
    name: "12306",
    aliases: &["railway", "铁路12306"],
    about: "12306 (China Railway 中国铁路): stations, trains between two stations with tickets left, a train's stops and fares; read-only",
    home: "https://www.12306.cn",
    cookie_domains: &[],
    required_cookies: &[],
    caps: &[],
    choices: Choices::NONE,
    min_interval: Duration::from_secs(3),
    kind: PlatformKind::Local,
    token: "",
    env: &[],
    guest: "everything (stations, trains, train, price)",
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
    Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "ATP reads 12306 without an account; it does not log in, book or order",
    ))
  }

  fn logged_in(&self) -> bool {
    false
  }

  async fn run_extra(&self, command: Self::Extra) -> Result<Data> {
    extra::run(&self.api, command).await
  }
}
