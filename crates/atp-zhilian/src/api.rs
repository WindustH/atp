//! Zhilian transport. The site's pages talk over three hosts:
//!
//! - `fe-api.zhaopin.com`, the job search of www.zhaopin.com
//!   (`POST /c/i/search/positions`), sent with the page's `x-zp-*` headers
//!   and request ids; without them the list comes back empty with
//!   `isVerification: 1`;
//! - `m.zhaopin.com`, the mobile site: job, search and company pages
//!   rendered with their data (`__INITIAL_STATE__`, `__INITIAL_DATA__`)
//!   and its own `/api/…` (applying);
//! - `cgate.zhaopin.com`, the gateway of the account (`x-zp-at` and
//!   `x-zp-rt` headers carry the login).
//!
//! www and m sit behind Tencent EdgeOne, which may answer with its
//! checkbox check ("Security Verification") instead of a page:
//! `verification_required` with that page, opened in the browser.
//!
//! A login is the `at` and `rt` cookies of zhaopin.com.

use std::time::Duration;

use atp_core::http::{Method, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result, Value, ValueExt, json};
use jiff::Timestamp;

/// The site; links of the search point here.
pub const WWW: &str = "https://www.zhaopin.com";
/// The mobile site: job and company pages.
pub const M: &str = "https://m.zhaopin.com";
const FE_API: &str = "https://fe-api.zhaopin.com";
const GATE: &str = "https://cgate.zhaopin.com";
/// The cookies holding the login.
pub const AT: &str = "at";
pub const RT: &str = "rt";
/// The cookie of the browser's client id, which the search sends along.
const CLIENT_ID: &str = "x-zp-client-id";
/// The cookie of the app's device serial, sent as a header when present.
const DEVICE_SN: &str = "x-zp-device-sn";

pub struct Api {
  pub ctx: Ctx,
}

/// 32 random hex digits.
fn hex() -> String {
  let b: [u8; 16] = rand::random();
  b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A random version-4 UUID.
pub fn uuid() -> String {
  let mut b: [u8; 16] = rand::random();
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
  format!(
    "{}-{}-{}-{}-{}",
    &hex[..8],
    &hex[8..12],
    &hex[12..16],
    &hex[16..20],
    &hex[20..]
  )
}

/// A random `_v`, as the pages make it: `0.43240637`.
fn version_tag() -> String {
  format!("{:.8}", rand::random::<f64>())
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self { ctx }
  }

  fn cookie(&self, name: &str) -> Option<String> {
    self.ctx.http.cookie(name).filter(|v| !v.is_empty())
  }

  pub fn require_login(&self) -> Result<()> {
    self.ctx.require_login(&[AT, RT])
  }

  /// The search of www's 搜索 page: the body as the page sends it; the
  /// answer's `data`.
  pub async fn search(&self, body: &Value, referer: &str) -> Result<Value> {
    let client = self.cookie(CLIENT_ID).unwrap_or_else(uuid);
    let page_request = format!(
      "{}-{}-{}",
      hex(),
      Timestamp::now().as_millisecond(),
      rand::random_range(100_000..1_000_000)
    );
    let resp = self
      .ctx
      .http
      .post(format!("{FE_API}/c/i/search/positions"))
      .query("_v", version_tag())
      .query("x-zp-page-request-id", page_request)
      .query("x-zp-client-id", client)
      .header("accept", "application/json, text/plain, */*")
      .header("origin", WWW)
      .header("referer", referer)
      .header("x-zp-page-code", "4019")
      .header("x-zp-platform", "13")
      .header("x-zp-business-system", "1")
      .json(body)
      .send()
      .await?;
    let v = self.answer(resp)?;
    self.data(v)
  }

  /// A page of the mobile site as HTML.
  pub async fn page(&self, url: &str) -> Result<String> {
    let resp = self
      .ctx
      .http
      .get(url)
      .header(
        "accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
      )
      .header("referer", format!("{M}/"))
      .send()
      .await?;
    let text = resp.text();
    check_edgeone(&text, url)?;
    if resp.status.as_u16() == 404 {
      return Err(Error::not_found(format!("Zhilian has no page {url}")));
    }
    Ok(resp.check()?.text())
  }

  /// A call to the gateway of the account, with the login in the headers
  /// as the site's scripts send it.
  pub async fn gate(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
    self.require_login()?;
    let (at, rt) = (
      self.cookie(AT).unwrap_or_default(),
      self.cookie(RT).unwrap_or_default(),
    );
    let mut req = self
      .ctx
      .http
      .request(method, format!("{GATE}{path}"))
      .query("at", &at)
      .query("rt", &rt)
      .header("accept", "application/json, text/plain, */*")
      .header("origin", M)
      .header("referer", format!("{M}/"))
      .header("x-zp-at", &at)
      .header("x-zp-rt", &rt);
    if let Some(body) = body {
      req = req.json(body);
    }
    let v = self.answer(req.send().await?)?;
    self.data(v)
  }

  /// A change through the mobile site's `/api`, from the page `referer`
  /// (`page_code` its code), after a pause of a few seconds as a person
  /// would take. The fields the site's request library adds come along.
  pub async fn m_post(
    &self,
    path: &str,
    body: Value,
    referer: &str,
    page_code: &str,
  ) -> Result<Value> {
    self.require_login()?;
    self
      .ctx
      .http
      .pause(Duration::from_secs(2), Duration::from_secs(5))
      .await;
    let device = uuid();
    let tag = version_tag();
    let mut body = body;
    for (key, value) in [
      ("channel", json!("")),
      ("utmsource", json!("")),
      ("platform", json!(7)),
      ("d", json!(device)),
      ("version", json!("0.0.0")),
      ("_v", json!(tag)),
    ] {
      body[key] = value;
    }
    let resp = self
      .ctx
      .http
      .post(format!("{M}{path}"))
      .queries([
        ("channel", ""),
        ("utmsource", ""),
        ("platform", "7"),
        ("d", device.as_str()),
        ("version", "0.0.0"),
        ("_v", tag.as_str()),
      ])
      .header("accept", "application/json, text/plain, */*")
      .header("origin", M)
      .header("referer", referer)
      .header("x-zp-page-code", page_code)
      .header("x-zp-action-id", "")
      .header("x-zp-device-sn", self.cookie(DEVICE_SN).unwrap_or_default())
      .json(&body)
      .send()
      .await?;
    let text = resp.text();
    check_edgeone(&text, referer)?;
    self.answer(resp)
  }

  /// The JSON of a response, or the error it stands for.
  fn answer(&self, resp: Resp) -> Result<Value> {
    if let Ok(v) = serde_json::from_slice::<Value>(&resp.body) {
      return Ok(v);
    }
    let text = resp.text();
    check_edgeone(&text, WWW)?;
    if !resp.status.is_success() {
      return Err(status_error(resp.status, &text));
    }
    Err(Error::upstream(format!(
      "Zhilian answered with a page instead of data: {}",
      truncate(&one_line(&text), 160)
    )))
  }

  /// The `data` of `{code, data}` or `{statusCode, data}`, or the error.
  pub fn data(&self, v: Value) -> Result<Value> {
    match v.first_str(&["code", "statusCode"]).as_deref() {
      Some("200") => Ok(v.at("data").clone()),
      _ => Err(self.error(&v)),
    }
  }

  /// Zhilian's `{code, message}` as a shared error.
  pub fn error(&self, v: &Value) -> Error {
    let code = v
      .first_str(&["code", "statusCode", "apiCode"])
      .unwrap_or_default();
    let message = v
      .first_str(&["message", "statusDescription", "msg"])
      .unwrap_or_else(|| format!("code {code}"));
    let text = format!("Zhilian: {message}");
    match code.as_str() {
      "401" | "403" | "6000" | "6001" => {
        Error::auth(format!("{text} (the login is missing or ended)"))
          .with_hint(self.ctx.login_hint())
      }
      "2024" => Error::verification(format!(
        "{text}: the site wants its security check (安全认证) before going on"
      ))
      .with_url(format!("{M}/mapp/security-certify")),
      _ if message.contains("登录") => {
        Error::auth(format!("{text} (the login is missing or ended)"))
          .with_hint(self.ctx.login_hint())
      }
      _ => Error::upstream(format!("{text} ({code})")),
    }
  }
}

/// Tencent EdgeOne's checks in place of a page: the checkbox
/// ("Security Verification") or a script that sets its cookies
/// (`EO_Bot_Ssid`, `__tst_status`) and reloads, which a browser runs.
fn check_edgeone(text: &str, url: &str) -> Result<()> {
  let checkbox = text.contains("TEOCaptchaWidget") || text.contains("Tencent Cloud EdgeOne");
  let script = text.len() < 4096 && (text.contains("EO_Bot_Ssid") || text.contains("__tst_status"));
  if checkbox || script {
    return Err(
      Error::verification(
        "Zhilian's firewall (Tencent EdgeOne) wants a browser to pass its check first",
      )
      .with_url(url),
    );
  }
  Ok(())
}
