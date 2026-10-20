//! 知识星球 transport. Everything is read from the JSON API the web app
//! (wx.zsxq.com) calls, `api.zsxq.com/v2/…` and `/v3/…`, with the login's
//! cookies (`zsxq_access_token` among them) and the headers the app signs
//! its requests with ([`crate::sign`]). Answers are `{succeeded,
//! resp_data}` or `{succeeded: false, code, info}`.
//!
//! The public pages of a group (public.zsxq.com) read
//! `pub-api.zsxq.com/v2/…`, which answers without an account.
//!
//! Codes become shared errors ([`refusal`]): an ended login is
//! `not_authenticated`, a group you are not a member of (or no longer)
//! `permission_denied`, a deleted topic `not_found`, a refused signature
//! (1059) `signature_error`, HTTP 429 `rate_limited`. A refused signature
//! is asked once more with a new one; nothing else is retried.

use std::cell::RefCell;
use std::time::Duration;

use atp_core::http::{Method, Req, Resp, status_error};
use atp_core::text::{encode_uri_component, one_line, truncate};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt};

use crate::sign;

/// The API of the web app.
pub const API: &str = "https://api.zsxq.com";
/// The API of the public group pages.
pub const PUBLIC: &str = "https://pub-api.zsxq.com";
/// The web app; links point here.
pub const WEB: &str = "https://wx.zsxq.com";
/// The cookie of a login.
pub const LOGIN: &[&str] = &["zsxq_access_token"];
/// Session extra: the browser id of `X-Aduid`.
const ADUID: &str = "aduid";

pub struct Api {
  pub ctx: Ctx,
  /// The ids of the groups you joined, once asked in this run.
  groups: RefCell<Option<Vec<String>>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      groups: RefCell::new(None),
    }
  }

  pub fn logged_in(&self) -> bool {
    LOGIN.iter().all(|c| self.ctx.http.has_cookie(c))
  }

  pub fn need_login(&self) -> Result<()> {
    if self.logged_in() {
      return Ok(());
    }
    Err(Error::auth("this needs a 知识星球 login").with_hint(self.ctx.login_hint()))
  }

  /// The joined groups' ids, when this run asked for them before.
  pub fn known_groups(&self) -> Option<Vec<String>> {
    self.groups.borrow().clone()
  }

  pub fn keep_groups(&self, ids: Vec<String>) {
    *self.groups.borrow_mut() = Some(ids);
  }

  /// `GET <API><path>?<params>`: the answer's `resp_data`.
  pub async fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
    let url = url(API, path, params)?;
    let resp = self.request(Method::GET, &url).send().await?;
    match self.check(resp) {
      Err(e) if e.code == ErrorCode::SignatureError => {
        let resp = self.request(Method::GET, &url).send().await?;
        self.check(resp)
      }
      other => other,
    }
  }

  /// `GET <PUBLIC><path>`, without the session: what the public pages show.
  pub async fn public(&self, path: &str) -> Result<Value> {
    let url = url(PUBLIC, path, &[])?;
    let resp = self
      .ctx
      .http
      .get(&url)
      .no_cookies()
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WEB)
      .header("referer", format!("{WEB}/"))
      .send()
      .await?;
    self.check(resp)
  }

  /// A write (`POST`, `PUT`, `DELETE`) with the `{req_data}` body the app
  /// sends: needs the login, waits a few seconds first, as a person would
  /// between actions, and is never repeated.
  pub async fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
    self.need_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    let url = url(API, path, &[])?;
    let mut req = self.request(method, &url).retries(0);
    if let Some(body) = body {
      req = req.json(body);
    }
    let resp = req.send().await?;
    self.check(resp)
  }

  fn request(&self, method: Method, url: &str) -> Req<'_> {
    let mut req = self
      .ctx
      .http
      .request(method, url)
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WEB)
      .header("referer", format!("{WEB}/"));
    for (name, value) in sign::headers(url, &self.aduid()) {
      req = req.header(name, value);
    }
    req
  }

  /// The browser id of `X-Aduid`, made once and kept with the session.
  fn aduid(&self) -> String {
    if let Some(id) = self.ctx.extra(ADUID) {
      return id;
    }
    let id = sign::random_id();
    self.ctx.set_extra(ADUID, &id);
    id
  }

  /// The `resp_data` of an answer, or the error it stands for.
  fn check(&self, resp: Resp) -> Result<Value> {
    let status = resp.status.as_u16();
    let Ok(v) = serde_json::from_slice::<Value>(&resp.body) else {
      return Err(match status {
        401 | 429 => self.refusal(i64::from(status), ""),
        _ if !resp.status.is_success() => status_error(resp.status, &resp.text()),
        _ => Error::upstream(format!(
          "知识星球 answered with something other than data: {}",
          truncate(&one_line(&resp.text()), 160)
        )),
      });
    };
    if v.bool("succeeded") == Some(true) {
      return Ok(v.at("resp_data").clone());
    }
    let code = v.i64("code").unwrap_or(i64::from(status));
    let info = v
      .first_str(&["info", "error"])
      .filter(|i| i != "Unauthorized")
      .unwrap_or_default();
    Err(self.refusal(code, &info))
  }

  /// A refusal of the API as a shared error, with what to do about it.
  fn refusal(&self, code: i64, info: &str) -> Error {
    let meaning = meaning(code);
    let text = match (info.trim(), meaning) {
      ("", Some(m)) => format!("知识星球: {m} (code {code})"),
      ("", None) => format!("知识星球 refused the request (code {code})"),
      (info, _) => format!("知识星球: {info} (code {code})"),
    };
    let kind = kind(code);
    let error = Error::new(kind, text);
    match kind {
      ErrorCode::NotAuthenticated => error.with_hint(self.ctx.login_hint()),
      ErrorCode::PermissionDenied if matches!(code, 1005 | 1008 | 1037 | 10305) => error.with_hint(
        "知识星球 shows a group's topics to its members only: join it in the app or at \
         wx.zsxq.com (ATP neither joins nor pays); `atp zsxq group GROUP` shows its public preview",
      ),
      ErrorCode::PermissionDenied if matches!(code, 1027 | 11103) => {
        error.with_hint("renew the membership in the app or at wx.zsxq.com")
      }
      ErrorCode::SignatureError => {
        error.with_hint("retry later; if it keeps failing, update all-the-platforms")
      }
      ErrorCode::RateLimited => {
        error.with_hint("wait a while before retrying, and ask for fewer pages at a time")
      }
      _ => error,
    }
  }
}

/// Codes of an ended or refused login.
const AUTH: &[i64] = &[401, 1009, 1015];
/// Codes of what the account may not do: not a member (1005, 1008,
/// 10305), left (1037) or expired (1027, 11103), muted (1023), banned
/// (1066, 10009), no phone number (1028, 1029), trial limits (1036,
/// 1039), groups that forbid it (1046, 1047) …
const DENIED: &[i64] = &[
  403, 1005, 1006, 1008, 1011, 1019, 1023, 1027, 1028, 1029, 1030, 1035, 1036, 1037, 1038, 1039,
  1046, 1047, 1048, 1066, 1081, 10009, 10305, 11103, 19002, 50002, 50501, 50601, 50602,
];
/// Codes of deleted or unknown topics, comments, files, users and groups.
const MISSING: &[i64] = &[
  404, 1007, 1022, 1031, 1041, 1042, 1044, 1049, 10013, 10304, 11102, 11305, 12001, 18101, 19101,
  50101, 50102,
];
/// Codes of requests the API finds malformed (times, counts, sorts,
/// empty or refused content).
const INVALID: &[i64] = &[
  1001, 1002, 1003, 1004, 1010, 1012, 1014, 1018, 1020, 1021, 1026, 1033, 1043, 15307, 21502, 21503,
];

/// The shared error a code of the API stands for.
fn kind(code: i64) -> ErrorCode {
  let is = |codes: &[i64]| codes.contains(&code);
  match code {
    _ if is(AUTH) => ErrorCode::NotAuthenticated,
    _ if is(DENIED) => ErrorCode::PermissionDenied,
    _ if is(MISSING) => ErrorCode::NotFound,
    _ if is(INVALID) => ErrorCode::InvalidInput,
    1059 => ErrorCode::SignatureError,
    429 => ErrorCode::RateLimited,
    _ => ErrorCode::UpstreamError,
  }
}

/// What the web app says about a code, for answers that leave `info` empty.
fn meaning(code: i64) -> Option<&'static str> {
  Some(match code {
    401 => "not logged in, or the login has ended",
    403 | 1030 => "not allowed (没有权限)",
    404 => "nothing found there",
    429 => "too many requests (请勿频繁操作)",
    1005 | 1008 | 10305 => "you are not a member of this group",
    1007 | 18101 => "the topic was deleted",
    1009 => "the login was ended elsewhere (用户被踢下线)",
    1015 => "the login was not accepted (身份验证失败)",
    1022 => "the comment does not exist or was deleted",
    1023 => "you are muted in this group (禁言)",
    1027 | 11103 => "your membership of this group has expired",
    1028 | 1029 => "bind a phone number to the account first",
    1036 => "a trial card shows the last 30 days only",
    1037 => "you have left this group",
    1039 => "free trial members may not do this",
    1041 => "no such topic",
    1046 => "the group is marked high-risk; this is not allowed",
    1047 => "the group does not allow sharing its topics",
    1048 => "writes are not accepted at the moment",
    1049 => "the topic is under review",
    1059 => "the request's signature was refused (设备状态异常)",
    1066 | 10009 => "the account is banned",
    1081 => "this member's topics are not shown to you",
    11102 | 50102 => "no such group",
    1026 => "the content was refused (内容非法)",
    15307 => "the text is empty",
    _ => return None,
  })
}

/// `<base><path>?<params>` with the values encoded as the app encodes them
/// (`encodeURIComponent`, and `'` as `%27`), in the form it is sent and
/// signed.
fn url(base: &str, path: &str, params: &[(&str, String)]) -> Result<String> {
  let mut url = format!("{base}{path}");
  for (i, (key, value)) in params.iter().enumerate() {
    url.push(if i == 0 { '?' } else { '&' });
    url.push_str(key);
    url.push('=');
    url.push_str(&encode_uri_component(value).replace('\'', "%27"));
  }
  url::Url::parse(&url)
    .map(String::from)
    .map_err(|e| Error::internal(format!("bad url {url}: {e}")))
}
