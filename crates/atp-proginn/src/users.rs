//! The logged-in account (`user/getInfo`), as the board's pages read it.

use atp_core::{Error, Result, User};

use crate::api::Api;
use crate::parse;

pub async fn whoami(api: &Api) -> Result<User> {
  let v = api.own("user/getInfo").await?;
  parse::account(&v).ok_or_else(|| {
    Error::auth("Proginn knows no account for this session (it may have ended)")
      .with_hint(api.ctx.login_hint())
  })
}
