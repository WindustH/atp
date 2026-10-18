//! Careers transport. Every site is read through the requests its own
//! pages make, each backend with its own hosts and answers ([`crate::feishu`],
//! [`crate::tencent`], [`crate::alibaba`], [`crate::meituan`],
//! [`crate::kuaishou`], [`crate::beisen`], [`crate::moka`]); what they share
//! lives here: the site named by `--site`, pages (kept for the run, so a
//! page read to recognize a site is not read again), tokens a site hands
//! out for the run, and JSON answers with the errors their status or a
//! firewall's page stand for.
//!
//! Nothing here logs in. The sites' cookies (CSRF tokens, visitor
//! sessions) live for one run and are never saved.

use std::collections::HashMap;
use std::sync::Mutex;

use atp_core::http::{Req, Resp, status_error};
use atp_core::text::{one_line, truncate};
use atp_core::{Ctx, Error, Result, Value};

/// What browsers send for a page.
const HTML: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

/// Words of the pages firewalls and bot checks answer with instead of
/// data (Aliyun's WAF, Alibaba's baxia, Tencent's and others' captchas).
const CHECKS: &[&str] = &[
  "_waf_",
  "aliyun_waf",
  "punish",
  "baxia",
  "captcha",
  "TCaptcha",
  "x5secdata",
];

/// A page read in this run.
#[derive(Debug, Clone)]
pub struct Html {
  /// Where it was read after redirects.
  pub url: String,
  pub text: String,
}

pub struct Api {
  pub ctx: Ctx,
  /// Pages read in this run, by the link asked for.
  pages: Mutex<HashMap<String, Html>>,
  /// Tokens sites hand out for a visit (Feishu Hire's CSRF token,
  /// Alibaba's `_csrf`), by origin.
  tokens: Mutex<HashMap<String, String>>,
}

impl Api {
  pub fn new(ctx: Ctx) -> Self {
    Self {
      ctx,
      pages: Mutex::default(),
      tokens: Mutex::default(),
    }
  }

  /// The site `--site` names.
  pub fn site_arg(&self) -> Result<String> {
    self
      .ctx
      .site
      .clone()
      .filter(|s| !s.trim().is_empty())
      .ok_or_else(|| {
        Error::input(
          "which career site? Pass --site with a short name (bytedance, tencent, alibaba, \
           meituan, kuaishou) or the site's link",
        )
        .with_hint("`atp careers sites` lists the short names and the sites ATP recognizes")
      })
  }

  /// A page as HTML, read once per run.
  pub async fn page(&self, url: &str) -> Result<Html> {
    if let Some(page) = self.pages.lock().expect("pages").get(url) {
      return Ok(page.clone());
    }
    let resp = self.ctx.http.get(url).header("accept", HTML).send().await?;
    if resp.status.as_u16() == 404 {
      return Err(Error::not_found(format!("there is no page at {url}")));
    }
    let resp = resp.check()?;
    let page = Html {
      url: resp.url.clone(),
      text: resp.text(),
    };
    self.remember(url, &page);
    Ok(page)
  }

  /// Keep a page for the run under another link (the site it was found
  /// to be).
  pub fn remember(&self, url: &str, page: &Html) {
    self
      .pages
      .lock()
      .expect("pages")
      .insert(url.to_owned(), page.clone());
  }

  pub fn token(&self, origin: &str) -> Option<String> {
    self.tokens.lock().expect("tokens").get(origin).cloned()
  }

  pub fn set_token(&self, origin: &str, token: &str) {
    self
      .tokens
      .lock()
      .expect("tokens")
      .insert(origin.to_owned(), token.to_owned());
  }

  pub fn forget_token(&self, origin: &str) {
    self.tokens.lock().expect("tokens").remove(origin);
  }

  /// Send a request and read its JSON answer, or the error its status or
  /// page stands for. `page` is where a browser would pass a check.
  pub async fn json(&self, req: Req<'_>, page: &str) -> Result<Value> {
    let resp = req.send().await?;
    parse(&resp, page)
  }
}

/// The JSON of an answer, or the error it stands for.
pub fn parse(resp: &Resp, page: &str) -> Result<Value> {
  if let Ok(v) = serde_json::from_slice::<Value>(&resp.body) {
    return Ok(v);
  }
  let text = resp.text();
  if CHECKS.iter().any(|w| text.contains(w)) {
    return Err(
      Error::verification(format!(
        "{} answered with a bot check instead of data, which ATP does not pass",
        host(page)
      ))
      .with_url(page)
      .unclearable(),
    );
  }
  if !resp.status.is_success() {
    return Err(status_error(resp.status, &text));
  }
  Err(Error::upstream(format!(
    "{} answered with a page instead of data: {}",
    host(page),
    truncate(&one_line(&text), 160)
  )))
}

/// The name a cached value of a site goes under: what it is and the
/// site's link, as a file name (`feishu-jobs_bytedance_com_campus`).
pub fn cache_key(what: &str, link: &str) -> String {
  let link = link
    .trim_start_matches("https://")
    .trim_start_matches("http://")
    .replace(|c: char| !c.is_ascii_alphanumeric(), "_");
  format!("{what}-{link}")
}

/// The host of a link, for messages.
pub fn host(url: &str) -> String {
  url::Url::parse(url)
    .ok()
    .and_then(|u| u.host_str().map(str::to_owned))
    .unwrap_or_else(|| url.to_owned())
}

/// A site's own error as a shared one: `what` names the site.
pub fn upstream(what: &str, message: Option<String>, code: impl std::fmt::Display) -> Error {
  match message.filter(|m| !m.trim().is_empty()) {
    Some(message) => Error::upstream(format!("{what}: {message} ({code})")),
    None => Error::upstream(format!("{what} answered with code {code}")),
  }
}
