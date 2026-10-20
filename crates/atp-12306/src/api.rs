//! 12306 transport: the JSON its query page calls on kyfw.12306.cn
//! (`/otn/leftTicket/…`, `/otn/czxx/…`), the train-number search of
//! search.12306.cn, and the station list script. Nothing is signed and
//! no account is involved.
//!
//! Like the site, a run first opens the query page (`/otn/leftTicket/init`)
//! once: it sets the anonymous session cookies the JSON calls want, and
//! names the current path of the ticket query (`CLeftTicketUrl`), which
//! 12306 changes from time to time. 12306 answers what it will not serve
//! with a redirect to an error page or a web page instead of JSON; both
//! are errors here, never data.

use std::cell::{Cell, RefCell};
use std::sync::LazyLock;

use atp_core::http::{Resp, status_error};
use atp_core::{Ctx, Error, ErrorCode, Result, Value, ValueExt};
use regex::Regex;

pub const KYFW: &str = "https://kyfw.12306.cn";
pub const SEARCH: &str = "https://search.12306.cn";
const INIT: &str = "/otn/leftTicket/init";
/// The ticket query's path when the page does not name one.
const QUERY_PATH: &str = "leftTicket/queryG";

static QUERY_URL: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"CLeftTicketUrl\s*=\s*['"]([A-Za-z/]+)['"]"#).unwrap());

/// Query parameters.
pub type Params = Vec<(&'static str, String)>;

pub struct Api {
  pub ctx: Ctx,
  /// Whether the query page was opened in this run.
  opened: Cell<bool>,
  /// The ticket query's path (`leftTicket/queryG`).
  query: RefCell<String>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      opened: Cell::new(false),
      query: RefCell::new(QUERY_PATH.to_owned()),
    }
  }

  /// Open the query page once: session cookies and the query's path.
  async fn open(&self) -> Result<()> {
    if self.opened.get() {
      return Ok(());
    }
    let resp = self.ctx.http.get(format!("{KYFW}{INIT}")).send().await?;
    if refused_page(&resp) {
      return Err(turned_away());
    }
    if !resp.status.is_success() {
      return Err(status_error(resp.status, &resp.text()));
    }
    if let Some(path) = QUERY_URL.captures(&resp.text()).map(|c| c[1].to_owned()) {
      *self.query.borrow_mut() = path;
    }
    self.opened.set(true);
    Ok(())
  }

  /// The ticket query's current path.
  pub fn query_path(&self) -> String {
    self.query.borrow().clone()
  }

  /// Use another path for the ticket query (12306's `c_url` answer).
  pub fn set_query_path(&self, path: &str) {
    *self.query.borrow_mut() = path.trim_start_matches('/').to_owned();
  }

  /// `GET <kyfw>/otn/<path>` as the query page sends it; the answer as
  /// JSON, whatever its `status`.
  pub async fn otn(&self, path: &str, params: Params) -> Result<Value> {
    self.open().await?;
    let resp = self
      .ctx
      .http
      .get(format!("{KYFW}/otn/{path}"))
      .header("referer", format!("{KYFW}{INIT}"))
      .header("x-requested-with", "XMLHttpRequest")
      .header("accept", "application/json, text/javascript, */*; q=0.01")
      .queries(params)
      .send()
      .await?;
    json(&resp)
  }

  /// `GET <search>/<path>`: the train-number search.
  pub async fn search(&self, path: &str, params: Params) -> Result<Value> {
    let resp = self
      .ctx
      .http
      .get(format!("{SEARCH}/{path}"))
      .header("referer", format!("{KYFW}/"))
      .queries(params)
      .send()
      .await?;
    json(&resp)
  }

  /// A plain file of the site (the station list script).
  pub async fn file(&self, path: &str) -> Result<String> {
    let resp = self.ctx.http.get(format!("{KYFW}{path}")).send().await?;
    if refused_page(&resp) {
      return Err(turned_away());
    }
    Ok(resp.check()?.text())
  }
}

/// An answer that ended on 12306's error page (`…/error.html`).
fn refused_page(resp: &Resp) -> bool {
  resp.url.contains("error.html") || resp.url.contains("/mormhweb/logFiles/")
}

fn turned_away() -> Error {
  Error::new(
    ErrorCode::RateLimited,
    "12306 turned the request away (its error page)",
  )
  .with_hint("12306 refuses frequent queries and some networks; wait a few minutes and retry")
}

/// The body as JSON; an error page or a web page is an error.
fn json(resp: &Resp) -> Result<Value> {
  if refused_page(resp) {
    return Err(turned_away());
  }
  if !resp.status.is_success() {
    return Err(status_error(resp.status, &resp.text()));
  }
  serde_json::from_slice(&resp.body).map_err(|_| {
    let page = resp.text().trim_start().starts_with('<');
    let what = if page { "a web page" } else { "something else" };
    Error::upstream(format!("12306 answered with {what} instead of data"))
      .with_hint("12306 sometimes serves a busy page; wait a minute and retry")
  })
}

/// A JSON answer that must have `status: true`; its `messages` otherwise.
pub fn ok(v: Value) -> Result<Value> {
  if v.bool("status") == Some(true) {
    return Ok(v);
  }
  let messages: Vec<String> = match v.at("messages") {
    Value::Array(list) => list
      .iter()
      .filter_map(|m| m.as_str().map(str::to_owned))
      .collect(),
    other => other.as_str().map(str::to_owned).into_iter().collect(),
  };
  let text = messages.join("; ");
  if text.is_empty() {
    return Err(Error::upstream(
      "12306 refused the query without saying why",
    ));
  }
  Err(Error::input(format!("12306: {text}")))
}
