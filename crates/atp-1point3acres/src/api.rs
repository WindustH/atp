//! 1Point3Acres transport: the JSON API the site's own pages call
//! (`https://api.1point3acres.com/api/…`), and for the account its tRPC
//! service (`https://trpc.1point3acres.com/trpc/…`). The forum pages
//! themselves (`www.1point3acres.com/bbs/…`, `/home/…`) sit behind a
//! Cloudflare challenge and are never read.
//!
//! Lists, threads and their replies need no account. A login is the
//! site's own cookies of `.1point3acres.com` (Discuz's `…_auth` among
//! them), sent along as the web pages send them; the API answers HTTP 401
//! `请先登录` without them (search, profiles, users' threads, all answers
//! to a reply). Answers are `{errno, msg, …}`: `errno` 0 (or `msg` `OK`)
//! is success.
//!
//! A Cloudflare challenge, HTTP 429, a refusal or a page instead of JSON
//! become `verification_required` (never answered; a browser pass would
//! not carry over to these requests), `rate_limited`, `not_authenticated`
//! and the like; nothing else is parsed in place of the data. Nothing is
//! retried: a refusal is not worth repeating at once.

use std::time::Duration;

use atp_core::http::{Method, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt, json};

/// The site; links point here.
pub const WWW: &str = "https://www.1point3acres.com";
/// The API the web pages call.
pub const API: &str = "https://api.1point3acres.com";
/// The tRPC service the web pages ask who is logged in.
const TRPC: &str = "https://trpc.1point3acres.com/trpc";
/// The end of Discuz's login cookie (`<prefix>_auth`).
const AUTH_COOKIE: &str = "_auth";

pub struct Api {
  pub ctx: Ctx,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  /// Whether the session holds Discuz's login cookie.
  pub fn logged_in(&self) -> bool {
    self
      .ctx
      .http
      .cookies()
      .iter()
      .any(|(name, value)| name.ends_with(AUTH_COOKIE) && !value.is_empty())
  }

  /// `GET <API><path>?<params>`.
  pub async fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    let resp = self
      .request(Method::GET, &format!("{API}{path}"))
      .queries(params.iter().map(|(k, v)| (*k, v)))
      .send()
      .await?;
    self.check(resp)
  }

  /// `POST <API><path>` with a JSON body: needs the login and waits a few
  /// seconds first, as a person would between actions.
  pub async fn post(&self, path: &str, body: &Value) -> Result<Value> {
    if !self.logged_in() {
      return Err(Error::auth("this needs a 1Point3Acres login").with_hint(self.ctx.login_hint()));
    }
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    let resp = self
      .request(Method::POST, &format!("{API}{path}"))
      .json(body)
      .send()
      .await?;
    self.check(resp)
  }

  /// A tRPC query without input, as the web pages batch it; its `json`.
  pub async fn trpc(&self, procedure: &str) -> Result<Value> {
    let input = json!({"0": {"json": null, "meta": {"values": ["undefined"]}}});
    let resp = self
      .request(Method::GET, &format!("{TRPC}/{procedure}"))
      .query("batch", 1)
      .query("input", input)
      .send()
      .await?;
    let v = self.check(resp)?;
    let first = v.at("0");
    if !first.at("error").is_null() {
      let message = first
        .str("error.json.message")
        .unwrap_or_else(|| "error".into());
      return Err(self.refusal(
        first.u64("error.json.data.httpStatus").unwrap_or(0) as u16,
        &message,
      ));
    }
    Ok(first.at("result.data.json").clone())
  }

  fn request(&self, method: Method, url: &str) -> atp_core::http::Req<'_> {
    self
      .ctx
      .http
      .request(method, url)
      .retries(0)
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WWW)
      .header("referer", format!("{WWW}/"))
  }

  /// The JSON answer, or the error it stands for.
  fn check(&self, resp: Resp) -> Result<Value> {
    if challenged(&resp) {
      return Err(
        Error::verification(
          "1Point3Acres's Cloudflare check stopped the request; ATP does not pass it",
        )
        .with_url(format!("{WWW}/bbs/"))
        .with_hint("retry later, or from another network")
        .unclearable(),
      );
    }
    let status = resp.status.as_u16();
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      if status == 429 {
        return Err(rate_limited("HTTP 429, too many requests"));
      }
      if !resp.status.is_success() {
        return Err(status_error(resp.status, &resp.text()));
      }
      return Err(Error::upstream(format!(
        "1Point3Acres answered with a page instead of data: {}",
        truncate(&one_line(&resp.text()), 160)
      )));
    };
    let errno = v.i64("errno").unwrap_or(0);
    let message = v.str("msg").unwrap_or_default();
    if resp.status.is_success() && (errno == 0 || message == "OK") {
      return Ok(v);
    }
    let message = v
      .str("msg")
      .or_else(|| v.str("0.error.json.message"))
      .unwrap_or_else(|| format!("HTTP {status}"));
    Err(self.refusal(status, &message))
  }

  /// A refusal (`errno` other than 0, or an HTTP error) as a shared error.
  fn refusal(&self, status: u16, message: &str) -> Error {
    let text = format!("1Point3Acres: {message}");
    let has = |words: &[&str]| words.iter().any(|w| message.contains(w));
    if status == 401 || has(&["登录", "UNAUTHORIZED"]) {
      let text = if self.logged_in() {
        format!("{text} (the session ended)")
      } else {
        format!("{text} (this needs a login)")
      };
      return Error::auth(text).with_hint(self.ctx.login_hint());
    }
    if has(&["验证", "captcha", "人机"]) {
      return Error::verification(format!("{text}; ATP does not answer the site's captcha"))
        .with_url(WWW)
        .unclearable();
    }
    if status == 429 || has(&["频繁", "太快"]) {
      return rate_limited(message);
    }
    if status == 404 || has(&["不存在", "已删除", "被删除"]) {
      return Error::not_found(text);
    }
    match status {
      403 => Error::new(ErrorCode::PermissionDenied, text),
      400 | 422 => Error::input(text),
      _ => Error::upstream(text),
    }
  }
}

/// Cloudflare's challenge page instead of the answer.
fn challenged(resp: &Resp) -> bool {
  if resp.header("cf-mitigated") == Some("challenge") {
    return true;
  }
  let status = resp.status.as_u16();
  if !matches!(status, 403 | 503) {
    return false;
  }
  let text = resp.text();
  text.contains("Just a moment") || text.contains("challenge-platform")
}

fn rate_limited(why: &str) -> Error {
  Error::new(
    ErrorCode::RateLimited,
    format!("1Point3Acres refused the request ({why})"),
  )
  .with_hint("wait a while before retrying, and ask for fewer pages at a time")
}
