//! The logged-in account (`cgate …/getUserDetailHideMobileAndEmail`, which
//! leaves the phone number and e-mail address out) and companies, which
//! are Zhilian's users here (`m.zhaopin.com/company/<number>.htm`).

use atp_core::http::Method;
use atp_core::{Error, Result, User, Value, ValueExt, json};

use crate::api::Api;
use crate::jobs::state;
use crate::{parse, refs};

/// The account as the gateway tells it: id, name, avatar and résumés.
pub async fn account(api: &Api) -> Result<Value> {
  api
    .gate(
      Method::GET,
      "/resumeapi/businessservice/getUserDetailHideMobileAndEmail",
      None,
    )
    .await
}

pub async fn whoami(api: &Api) -> Result<User> {
  let v = account(api).await?;
  let id = v.str("id").ok_or_else(|| {
    Error::auth("Zhilian did not name the logged-in account").with_hint(api.ctx.login_hint())
  })?;
  let mut user = User {
    name: v.str("name").unwrap_or_else(|| id.clone()),
    avatar: v.str("headImg").filter(|a| a.starts_with("http")),
    raw: Some(parse::sanitize(&v)),
    id,
    ..User::default()
  };
  let resumes = v.list("resumes").len();
  user.extra.insert("resumes".into(), json!(resumes));
  Ok(user)
}

/// A company's mobile page.
pub async fn company(api: &Api, arg: &str) -> Result<User> {
  let number = refs::company(arg)?;
  let url = refs::company_url(&number);
  let html = api.page(&url).await?;
  let data = state(&html, "__INITIAL_STATE__").unwrap_or_default();
  parse::company_page(&data, &number)
    .ok_or_else(|| Error::not_found(format!("Zhilian shows no company at {url}")))
}
