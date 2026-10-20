//! Pixiv transport: the JSON API the site's own pages call
//! (`www.pixiv.net/ajax/…`, answering `{error, message, body}`), the
//! rankings' JSON (`ranking.php?format=json`, without that envelope), and
//! for the actions the requests the pages send, with the token the pages
//! carry (`x-csrf-token`).
//!
//! A login is the site's `PHPSESSID` cookie. Pixiv gives guests one too, a
//! bare hash; a logged-in one reads `<user id>_<hash>`, so only that form
//! counts as a login, and the account's number comes from it. Guests are
//! sent nothing but what the site set for them.
//!
//! HTTP 401 (and 403 for guests), 404 and 429 become `not_authenticated`,
//! `not_found` and `rate_limited`; Cloudflare's check in front of the site
//! is `verification_required`, which a browser pass does not clear for
//! ATP's requests. Reads are tried twice more on network errors and 5xx,
//! actions never.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use atp_core::http::{Method, Req, Resp};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt};
use regex::Regex;

/// The site; links point here, and its pages are the `Referer` of every
/// request (the image host refuses downloads without it).
pub const SITE: &str = "https://www.pixiv.net";
/// The cookie of a session.
pub const LOGIN: &[&str] = &["PHPSESSID"];

/// The token of the page's data, in its meta tag or its serialized state
/// (with escaped quotes).
static TOKEN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"token\\?"\s*:\s*\\?"([0-9a-f]{32})"#).unwrap());

/// Query parameters.
pub type Query = Vec<(&'static str, String)>;

pub struct Api {
  pub ctx: Ctx,
  /// The page's `x-csrf-token`, read once per run.
  token: RefCell<Option<String>>,
  /// `profile/all` by user, for the pages of one listing.
  profiles: RefCell<HashMap<String, Value>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      token: RefCell::new(None),
      profiles: RefCell::new(HashMap::new()),
    }
  }

  /// The account's number, from a logged-in `PHPSESSID` (`<uid>_<hash>`).
  pub fn uid(&self) -> Option<String> {
    let session = self.ctx.http.cookie(LOGIN[0])?;
    let (uid, rest) = session.split_once('_')?;
    let numeric = !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit());
    (numeric && !rest.is_empty()).then(|| uid.to_owned())
  }

  pub fn logged_in(&self) -> bool {
    self.uid().is_some()
  }

  /// The account's number, or `not_authenticated`.
  pub fn need_login(&self) -> Result<String> {
    self
      .uid()
      .ok_or_else(|| Error::auth("this needs a pixiv login").with_hint(self.ctx.login_hint()))
  }

  /// `GET <SITE><path>`: the `body` of the answer.
  pub async fn get(&self, path: &str, query: Query) -> Result<Value> {
    let resp = self
      .request(Method::GET, path)
      .header("accept", "application/json")
      .queries(query)
      .query("lang", "en")
      .send()
      .await?;
    Ok(self.check(&resp)?.at("body").clone())
  }

  /// `GET` of JSON that comes without the envelope (the rankings).
  pub async fn get_plain(&self, path: &str, query: Query) -> Result<Value> {
    let resp = self
      .request(Method::GET, path)
      .header("accept", "application/json")
      .queries(query)
      .send()
      .await?;
    self.check(&resp)
  }

  /// An action with a JSON body, as the work pages send it: needs the
  /// login and the page's token, and waits a moment first.
  pub async fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
    let req = self.action(path).await?.json(body);
    let resp = req.send().await?;
    Ok(self.check(&resp)?.at("body").clone())
  }

  /// An action with a form body (the older endpoints of follows and
  /// bookmark removal), answered with JSON of any shape or nothing; an
  /// answer with `error: true` is a refusal.
  pub async fn post_form(&self, path: &str, form: &[(&str, String)]) -> Result<Value> {
    let req = self
      .action(path)
      .await?
      .form(form.iter().map(|(k, v)| (*k, v)));
    let resp = req.send().await?;
    let status = resp.status.as_u16();
    if resp.url.contains("accounts.pixiv.net") {
      return Err(self.refusal(401, "the site asks to log in again"));
    }
    let v: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
    if resp.status.is_success() && v.bool("error") != Some(true) {
      return Ok(v);
    }
    if v.is_null() {
      return Err(self.not_json(&resp));
    }
    Err(self.refusal(status, &message(&v, status)))
  }

  async fn action(&self, path: &str) -> Result<Req<'_>> {
    self.need_login()?;
    let token = self.token().await?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(1), Duration::from_secs(3))
      .await;
    Ok(
      self
        .request(Method::POST, path)
        .header("accept", "application/json")
        .header("origin", SITE)
        .header("x-csrf-token", token),
    )
  }

  /// The `x-csrf-token` of the session, from the front page's data.
  async fn token(&self) -> Result<String> {
    if let Some(token) = self.token.borrow().clone() {
      return Ok(token);
    }
    let resp = self
      .request(Method::GET, "/")
      .header("accept", "text/html,application/xhtml+xml,*/*;q=0.8")
      .send()
      .await?;
    let html = resp.text();
    if challenged(&resp, &html) {
      return Err(cloudflare());
    }
    let token = TOKEN
      .captures(&html)
      .map(|c| c[1].to_owned())
      .ok_or_else(|| {
        Error::upstream(
          "pixiv's front page carries no token for actions; the site may have changed",
        )
      })?;
    *self.token.borrow_mut() = Some(token.clone());
    Ok(token)
  }

  /// A user's works by kind (`illusts`, `manga`, `novels`: objects keyed
  /// by id), kept for the pages of one run.
  pub async fn profile(&self, uid: &str) -> Result<Value> {
    if let Some(v) = self.profiles.borrow().get(uid) {
      return Ok(v.clone());
    }
    let v = self
      .get(&format!("/ajax/user/{uid}/profile/all"), Vec::new())
      .await?;
    self.profiles.borrow_mut().insert(uid.to_owned(), v.clone());
    Ok(v)
  }

  fn request(&self, method: Method, path: &str) -> Req<'_> {
    let retries = if method == Method::GET { 2 } else { 0 };
    self
      .ctx
      .http
      .request(method, format!("{SITE}{path}"))
      .retries(retries)
      .header("referer", format!("{SITE}/"))
  }

  /// The JSON of an answer, or the error it stands for.
  fn check(&self, resp: &Resp) -> Result<Value> {
    let status = resp.status.as_u16();
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      return Err(self.not_json(resp));
    };
    let failed = match v.at("error") {
      Value::Bool(b) => *b,
      Value::String(s) => !s.is_empty(),
      _ => false,
    };
    if resp.status.is_success() && !failed {
      return Ok(v);
    }
    Err(self.refusal(status, &message(&v, status)))
  }

  fn not_json(&self, resp: &Resp) -> Error {
    let text = resp.text();
    if challenged(resp, &text) {
      return cloudflare();
    }
    if !resp.status.is_success() {
      return self.refusal(
        resp.status.as_u16(),
        &format!("HTTP {}", resp.status.as_u16()),
      );
    }
    Error::upstream(format!(
      "pixiv answered with a page instead of data: {}",
      truncate(&one_line(&text), 160)
    ))
  }

  /// A refusal of the API as a shared error.
  fn refusal(&self, status: u16, message: &str) -> Error {
    let text = format!("pixiv: {message}");
    let lower = message.to_lowercase();
    let has = |words: &[&str]| {
      words
        .iter()
        .any(|w| lower.contains(w) || message.contains(w))
    };
    if status == 401
      || (status == 403 && !self.logged_in())
      || has(&["log in", "login", "ログイン"])
    {
      return Error::auth(text).with_hint(self.ctx.login_hint());
    }
    if status == 429 || has(&["too many", "rate limit"]) {
      return Error::new(ErrorCode::RateLimited, text)
        .with_hint("wait a while before retrying, and ask for fewer pages at a time");
    }
    if status == 404
      || has(&[
        "deleted",
        "does not exist",
        "not found",
        "削除",
        "存在しない",
      ])
    {
      return Error::not_found(text);
    }
    if status == 403 {
      return Error::new(ErrorCode::PermissionDenied, text);
    }
    if status == 400 {
      return Error::input(text);
    }
    Error::upstream(text)
  }
}

/// The message of a refusal (`message`, or `error` when it is text).
fn message(v: &Value, status: u16) -> String {
  v.str("message")
    .or_else(|| v.at("error").as_str().map(str::to_owned))
    .filter(|m| !m.trim().is_empty())
    .unwrap_or_else(|| format!("HTTP {status}"))
}

/// Whether an answer is Cloudflare's check rather than the site.
fn challenged(resp: &Resp, text: &str) -> bool {
  let head = text.get(..4096).unwrap_or(text);
  matches!(resp.status.as_u16(), 403 | 503)
    && (head.contains("Just a moment")
      || head.contains("cf_chl")
      || head.contains("challenge-platform"))
}

fn cloudflare() -> Error {
  Error::verification("Cloudflare's check in front of pixiv stopped the request")
    .with_url(format!("{SITE}/"))
    .with_hint("try again later or from another network; ATP does not answer the check")
    .unclearable()
}
