//! Nowcoder transport. The site's pages talk over three routes:
//!
//! - the job side (牛聘), `https://www.nowcoder.com/np-api/…` ([`Route::Jobs`]):
//!   the job search, the 校招 schedule and saving jobs, sent with
//!   `X-Requested-With: XMLHttpRequest` as the pages' client does; forms
//!   carry lists as repeated keys;
//! - the community gateway, `https://gw-c.nowcoder.com/api/sparta/…`
//!   ([`Route::Gateway`]): posts, comments, search, hot lists, 面经, your
//!   saved jobs and your messages;
//! - the site itself, `https://www.nowcoder.com/…` ([`Route::Site`]): the
//!   older endpoints the pages still call for follows, likes, comments, the
//!   account and your applications.
//!
//! Every answer is `{code, msg, data}` (the gateway adds `success`), `code`
//! 0 when it went well; 999 (`need login`, `用户未登录`) is a call that
//! needs a login, 1125 a comment the site wants a captcha for. Job pages are
//! rendered on the server with their data in `window.__INITIAL_STATE__`.
//!
//! A login is the `t` cookie of nowcoder.com, which every route reads.

use std::time::Duration;

use atp_core::http::{Method, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result, Value, ValueExt};

/// The site; links point here.
pub const WWW: &str = "https://www.nowcoder.com";
/// The community gateway.
const GATEWAY: &str = "https://gw-c.nowcoder.com";
/// The cookie holding the login.
pub const TOKEN: &str = "t";
/// The code of answers that need a login.
const NEED_LOGIN: i64 = 999;
/// The code of a comment the site wants a captcha for.
const CAPTCHA: i64 = 1125;

/// Where a call goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
  /// `www.nowcoder.com/np-api`: jobs and the 校招 schedule.
  Jobs,
  /// `gw-c.nowcoder.com`: the community.
  Gateway,
  /// `www.nowcoder.com`: follows, likes, comments, the account.
  Site,
}

impl Route {
  fn base(self) -> String {
    match self {
      Self::Jobs => format!("{WWW}/np-api"),
      Self::Gateway => GATEWAY.to_owned(),
      Self::Site => WWW.to_owned(),
    }
  }
}

/// A request body.
pub enum Body<'a> {
  None,
  Form(&'a [(&'a str, String)]),
  Json(&'a Value),
}

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  pub fn token(&self) -> Option<String> {
    self.ctx.http.cookie(TOKEN).filter(|t| !t.is_empty())
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(&[TOKEN])
  }

  /// `GET` with query parameters; the answer's `data`.
  pub async fn get(&self, route: Route, path: &str, params: &[(&str, String)]) -> Result<Value> {
    self
      .call(Method::GET, route, path, params, Body::None)
      .await
  }

  /// `POST` a form.
  pub async fn form(&self, route: Route, path: &str, form: &[(&str, String)]) -> Result<Value> {
    self
      .call(Method::POST, route, path, &[], Body::Form(form))
      .await
  }

  /// `POST` JSON.
  pub async fn json(&self, route: Route, path: &str, body: &Value) -> Result<Value> {
    self
      .call(Method::POST, route, path, &[], Body::Json(body))
      .await
  }

  /// A read of your own (`GET`): needs the login.
  pub async fn own(&self, route: Route, path: &str, params: &[(&str, String)]) -> Result<Value> {
    self.require_login()?;
    self.get(route, path, params).await
  }

  /// A change to the account (`POST`): needs the login and waits a few
  /// seconds first, as a person would between actions.
  pub async fn write(
    &self,
    route: Route,
    path: &str,
    params: &[(&str, String)],
    body: Body<'_>,
  ) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    self.call(Method::POST, route, path, params, body).await
  }

  pub async fn call(
    &self,
    method: Method,
    route: Route,
    path: &str,
    params: &[(&str, String)],
    body: Body<'_>,
  ) -> Result<Value> {
    let mut req = self
      .ctx
      .http
      .request(method, format!("{}{path}", route.base()))
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WWW)
      .header("referer", format!("{WWW}/"));
    if route == Route::Jobs {
      req = req.header("x-requested-with", "XMLHttpRequest");
    }
    req = match body {
      Body::None => req,
      Body::Form(form) => req.form(form.iter().map(|(k, v)| (*k, v.as_str()))),
      Body::Json(v) => req.json(v),
    };
    let resp = req.send().await?;
    self.check(resp)
  }

  /// A page of the site as HTML.
  pub async fn page(&self, path: &str) -> Result<String> {
    let url = format!("{WWW}{path}");
    let resp = self
      .ctx
      .http
      .get(&url)
      .header(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      )
      .header("referer", format!("{WWW}/"))
      .send()
      .await?;
    if resp.status.as_u16() == 404 {
      return Err(Error::not_found(format!("Nowcoder has no page {url}")));
    }
    Ok(resp.check()?.text())
  }

  /// The `data` of an answer, or the error it stands for.
  fn check(&self, resp: Resp) -> Result<Value> {
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      if !resp.status.is_success() {
        return Err(status_error(resp.status, &resp.text()));
      }
      return Err(Error::upstream(format!(
        "Nowcoder answered with a page instead of data: {}",
        truncate(&one_line(&resp.text()), 160)
      )));
    };
    match v.i64("code") {
      Some(0) if v.bool("success") != Some(false) => Ok(v.at("data").clone()),
      Some(code) => Err(self.error(code, &v)),
      None if !resp.status.is_success() => Err(status_error(resp.status, &resp.text())),
      None => Err(Error::upstream(format!(
        "Nowcoder answered something unexpected: {}",
        truncate(&one_line(&resp.text()), 160)
      ))),
    }
  }

  /// Nowcoder's `{code, msg}` as a shared error.
  fn error(&self, code: i64, v: &Value) -> Error {
    let message = v
      .first_str(&["msg", "message"])
      .unwrap_or_else(|| format!("code {code}"));
    let text = format!("Nowcoder: {message}");
    match code {
      NEED_LOGIN => {
        let what = if self.token().is_some() {
          format!("{text} (the session ended)")
        } else {
          format!("{text} (this needs a login)")
        };
        Error::auth(what).with_hint(self.ctx.login_hint())
      }
      CAPTCHA => Error::verification(format!(
        "{text}: the site wants a captcha for this comment, which ATP does not pass"
      ))
      .unclearable(),
      _ => Error::upstream(format!("{text} ({code})")),
    }
  }
}
