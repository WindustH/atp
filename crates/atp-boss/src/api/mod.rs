//! BOSS Zhipin transport. The site's job seeker pages
//! (`www.zhipin.com/web/geek/…`) call its API under `/wapi/`, which
//! answers the job search and job pages only to requests carrying
//! `__zp_stoken__`: a cookie the site's security script (a
//! control-flow-flattened program that reads the browser) computes in
//! the page. ATP never runs, ports or computes that script, and never
//! sends that cookie itself. So:
//!
//! - the search, job and company pages go through the browser tab
//!   ([`tab`]): the tab opens the site's search page and reads the answer
//!   to the page's own request ([`Api::jobs`]; the next pages are asked
//!   from that page as its scroll asks for them, [`Api::in_tab`]), or
//!   opens a job's or company's page (past the site's security page,
//!   whose scripts set the cookie and come back) and reads what it shows
//!   ([`Api::page`]);
//! - the account's lists and actions ([`Api::get`], [`Api::post`]) go
//!   over plain HTTP with the headers the site's request library adds
//!   (`X-Requested-With`, `zp_token`: the `bst` cookie) and the session's
//!   cookies but not `__zp_*`; an answer that is the environment check
//!   (37) sends the same request through the tab, made by a page of the
//!   site with its cookies ([`Api::call`]);
//! - the QR login talks to the passport API over plain HTTP
//!   ([`crate::login`]), and the city pages of hot jobs answer plain
//!   requests ([`crate::hot`]).
//!
//! Answers are `{code, message, zpData}`, `code` 0 for success. 7 and 1011
//! mean logged out; 37 the environment check (no valid `__zp_stoken__`);
//! 9 too many requests; 24 an account in its recruiter role; 120–122 a
//! stale `zp_token` (122 is renewed with `POST /wapi/zppassport/set/zpToken`
//! and the request sent once more, as the site does); 5012 the site's
//! captcha page (`safe-validate`).

pub mod tab;

use std::cell::{Cell, RefCell};
use std::time::Duration;

use atp_core::http::{Method, Resp};
use atp_core::tab::NO_DISPLAY;
use atp_core::text::encode_uri_component;
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};
use jiff::Timestamp;

/// The site.
pub const WWW: &str = "https://www.zhipin.com";
/// The page the account's requests are made from: the job seeker's
/// 互动 page (chats, applications, saved jobs).
pub const GEEK: &str = "https://www.zhipin.com/web/geek/recommend";
/// The site's captcha page.
const SAFE_VALIDATE: &str = "https://www.zhipin.com/web/geek/safe-validate";
/// The cookies of a login.
pub const LOGIN: &[&str] = &["wt2", "zp_at"];
/// The cookie the `zp_token` header repeats.
const ZP_TOKEN: &str = "bst";
/// The cookies of the site's security script (`__zp_stoken__`, its seeds),
/// which plain requests leave out.
const SECURITY_COOKIES: &str = "__zp_";

/// Codes of `{code, message, zpData}`.
const ENVIRONMENT: i64 = 37;
const STALE_TOKEN: i64 = 122;

/// A request to the API, as the page `from` makes it.
pub struct Call<'a> {
  pub path: &'a str,
  pub query: &'a [(&'a str, String)],
  /// The form of a POST; a GET without one.
  pub form: Option<&'a [(&'a str, String)]>,
  /// The page the request is made from.
  pub from: &'a str,
}

pub struct Api {
  pub ctx: Ctx,
  /// Whether the tab shows a page of the site.
  opened: Cell<bool>,
  /// Whether the tab wanted a window but found no display.
  no_display: Cell<bool>,
  /// The `token` header of requests made in the tab: the page's token,
  /// once read.
  token: RefCell<Option<String>>,
  /// The account's id, once `whoami` has read it.
  pub me: RefCell<Option<String>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      opened: Cell::new(false),
      no_display: Cell::new(false),
      token: RefCell::new(None),
      me: RefCell::new(None),
    }
  }

  pub fn logged_in(&self) -> bool {
    LOGIN.iter().all(|c| self.ctx.http.has_cookie(c))
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(LOGIN)
  }

  /// A cookie of the session.
  pub fn cookie(&self, name: &str) -> Option<String> {
    self.ctx.http.cookie(name).filter(|v| !v.is_empty())
  }

  /// A GET of the API, as the page `from` makes it.
  pub async fn get(&self, path: &str, query: &[(&str, String)], from: &str) -> Result<Value> {
    let call = Call {
      path,
      query,
      form: None,
      from,
    };
    self.call(&call).await
  }

  /// A form POST to the API, as the page `from` makes it.
  pub async fn post(
    &self,
    path: &str,
    query: &[(&str, String)],
    form: &[(&str, String)],
    from: &str,
  ) -> Result<Value> {
    let call = Call {
      path,
      query,
      form: Some(form),
      from,
    };
    self.call(&call).await
  }

  /// A change to the account: needs the login and waits a few seconds
  /// first, as a person would between actions.
  pub async fn write(&self, call: &Call<'_>) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    self.call(call).await
  }

  /// A call of the API, its `zpData`: over plain HTTP, or, when the
  /// answer is the environment check, made by a page of the site in the
  /// tab. A stale `zp_token` (122) is renewed once, as the site's pages do.
  pub async fn call(&self, call: &Call<'_>) -> Result<Value> {
    let mut v = self.plain(call).await?;
    if code(&v) == Some(STALE_TOKEN) {
      let renew = Call {
        path: "/wapi/zppassport/set/zpToken",
        query: &[],
        form: Some(&[]),
        from: call.from,
      };
      let renewed = self.plain(&renew).await?;
      self.data(renewed, call.from)?;
      v = self.plain(call).await?;
    }
    if code(&v) == Some(ENVIRONMENT) {
      return self.in_tab(call).await;
    }
    self.data(v, call.from)
  }

  /// A call over plain HTTP, the whole answer. A redirect (to the site's
  /// security page) counts as the environment check.
  async fn plain(&self, call: &Call<'_>) -> Result<Value> {
    let method = if call.form.is_some() {
      Method::POST
    } else {
      Method::GET
    };
    let mut query = call.query.to_vec();
    query.push(("_", Timestamp::now().as_millisecond().to_string()));
    let mut req = self
      .ctx
      .http
      .request(method, format!("{WWW}{}", call.path))
      .no_cookies()
      .no_redirects()
      .header("accept", "application/json, text/plain, */*")
      .header("x-requested-with", "XMLHttpRequest")
      .header("referer", call.from)
      .queries(query.iter().map(|(k, v)| (*k, v.as_str())));
    let cookies = self.cookies();
    if !cookies.is_empty() {
      req = req.header("cookie", cookies);
    }
    if let Some(token) = self.cookie(ZP_TOKEN) {
      req = req.header("zp_token", token);
    }
    if let Some(form) = call.form {
      req = req
        .header("origin", WWW)
        .form(form.iter().map(|(k, v)| (*k, v.as_str())));
    }
    let resp = req.send().await?;
    self.keep(&resp);
    if resp.status.is_redirection() {
      return Ok(json!({"code": ENVIRONMENT, "message": "sent to the security check"}));
    }
    resp.check()?.value()
  }

  /// The session's cookies for plain requests: all but the security
  /// script's.
  pub fn cookies(&self) -> String {
    self
      .ctx
      .http
      .cookie_header()
      .split("; ")
      .filter(|c| !c.is_empty() && !c.starts_with(SECURITY_COOKIES))
      .collect::<Vec<_>>()
      .join("; ")
  }

  /// The cookies a plain answer sets (a renewed `bst` …), into the
  /// session; the security script's are left to the browser.
  pub fn keep(&self, resp: &Resp) {
    for header in resp.headers.get_all("set-cookie") {
      let Some((name, value)) = header
        .to_str()
        .ok()
        .and_then(|h| h.split(';').next())
        .and_then(|pair| pair.split_once('='))
      else {
        continue;
      };
      let name = name.trim();
      if !name.starts_with(SECURITY_COOKIES) {
        self.ctx.http.set_cookie(name, value.trim());
      }
    }
  }

  /// The `zpData` of an answer, or the error its code stands for.
  pub fn data(&self, v: Value, page: &str) -> Result<Value> {
    match code(&v) {
      Some(0) => Ok(v.at("zpData").clone()),
      _ => Err(self.error(&v, page)),
    }
  }

  /// BOSS Zhipin's `{code, message}` as a shared error.
  fn error(&self, v: &Value, page: &str) -> Error {
    let code = code(v).unwrap_or(-1);
    let message = v
      .str("message")
      .filter(|m| !m.is_empty())
      .unwrap_or_else(|| "no message".into());
    let text = format!("BOSS Zhipin: {message} (code {code})");
    match code {
      7 | 1011 | 120 | 121 | STALE_TOKEN => {
        let why = if self.logged_in() {
          "the session ended"
        } else {
          "this needs a login"
        };
        Error::auth(format!("{text}; {why}")).with_hint(self.ctx.login_hint())
      }
      ENVIRONMENT => self.turned_away(
        page,
        &format!("the site's environment check did not let the request through ({message})"),
      ),
      5012 => self.check(SAFE_VALIDATE, "the site asks for its captcha (安全验证)"),
      9 => Error::new(ErrorCode::RateLimited, format!("{text}: too many requests"))
        .with_hint("wait a while before the next command; ATP does not retry"),
      24 => Error::new(
        ErrorCode::PermissionDenied,
        format!("{text}: the account is in its recruiter role"),
      )
      .with_hint("switch the account to the job seeker role (牛人身份) in the BOSS app"),
      17 | 19 => Error::input(text),
      _ => Error::upstream(text),
    }
  }
}

/// The `code` of an answer.
pub fn code(v: &Value) -> Option<i64> {
  v.i64("code")
}

/// A query or form as the site's request library writes it.
fn encode(pairs: &[(&str, String)]) -> String {
  pairs
    .iter()
    .map(|(k, v)| format!("{k}={}", encode_uri_component(v)))
    .collect::<Vec<_>>()
    .join("&")
}

impl Api {
  /// The site's security check turning ATP's browser away. It checks the
  /// browser, not a person: Chrome driven over DevTools reports itself
  /// automated (`navigator.webdriver`) in a window too, so passing the
  /// page by hand in ATP's window is not offered (`unclearable`).
  pub fn turned_away(&self, page: &str, what: &str) -> Error {
    let hint = if self.no_display.get() {
      NO_DISPLAY
    } else {
      "the site turns away the browser ATP drives over DevTools (it reports itself automated, \
       with or without a window), and ATP does not disguise it; `hot` works without it"
    };
    Error::verification(format!("BOSS Zhipin: {what}"))
      .with_url(page)
      .with_hint(hint)
      .unclearable()
  }

  /// A page answering ATP's browser in place of its data, for the person
  /// to pass in a window of that browser; or, when the tab found no
  /// display for its window, the reason it may have been turned away.
  fn check(&self, page: &str, what: &str) -> Error {
    let hint = if self.no_display.get() {
      NO_DISPLAY
    } else {
      "run the command on a terminal (without --no-verify): ATP opens the page in a window of \
       its browser, where the site's own scripts finish; pass any check yourself, then press Enter"
    };
    Error::verification(format!("BOSS Zhipin: {what}"))
      .with_url(page)
      .with_hint(hint)
  }
}
