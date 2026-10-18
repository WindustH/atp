//! The browser behind a context's tab: started on first use on the
//! platform's kept profile, its one page attached over DevTools, and the
//! steps every use of the page is made of (opening an address, waiting for
//! a load, evaluating a script, taking the site's cookies back).

use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};
use tokio::sync::{Mutex, OnceCell};

use crate::chrome::cdp::Cdp;
use crate::chrome::{self, Browser, Launch, Profile};
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::json::ValueExt;
use crate::store::Store;
use crate::text::{one_line, truncate};

/// The profile of a platform's browser, kept between runs:
/// `<cache>/<platform>/browser`.
pub fn profile(store: &Store) -> PathBuf {
  store.cache_dir().join("browser")
}

/// The tab of a context: whether its platform may open one, and the
/// browser once opened.
#[derive(Default)]
pub(crate) struct Slot {
  pub(crate) allowed: bool,
  open: OnceCell<Session>,
}

impl Slot {
  pub(crate) async fn get(&self, ctx: &Ctx) -> Result<&Session> {
    if !self.allowed {
      return Err(Error::internal(format!(
        "{} reads nothing through a browser (Platform::BROWSER_TAB)",
        ctx.info.name
      )));
    }
    self.open.get_or_try_init(|| Session::open(ctx)).await
  }

  pub(crate) async fn close(&self, ctx: &Ctx) {
    if let Some(session) = self.open.get() {
      session.close(ctx).await;
    }
  }
}

/// The browser of one run, with its one page attached over DevTools.
pub(crate) struct Session {
  pub(super) inner: Mutex<Inner>,
}

pub(super) struct Inner {
  browser: Browser,
  pub(super) cdp: Cdp,
  /// The DevTools session of the page.
  pub(super) page: String,
  /// The address the page shows.
  pub(super) at: Option<String>,
  pub(super) closed: bool,
}

impl Session {
  async fn open(ctx: &Ctx) -> Result<Self> {
    let exe = chrome::find().ok_or_else(|| {
      Error::internal(format!(
        "{} is read through a browser: install Chrome, Chromium, Edge or Brave",
        ctx.info.name
      ))
      .with_hint("ATP_BROWSER names one by its path or command")
    })?;
    let proxy = ctx.http.proxy().and_then(|p| chrome::proxy_server(&p));
    let launch = Launch {
      profile: Profile::Kept(profile(&ctx.store)),
      headless: true,
      proxy: proxy.as_deref(),
    };
    let mut browser = Browser::launch(&exe, launch)?;
    let mut cdp = Cdp::connect(&browser.endpoint().await?).await?;
    let (cookies, domains) = (ctx.http.cookies(), ctx.info.cookie_domains);
    if browser.kept() {
      cdp
        .seed(&cookies, domains, ctx.info.required_cookies)
        .await?;
    } else {
      cdp.set_cookies(&cookies, domains).await?;
    }
    let target = match cdp.pages().await?.into_iter().next() {
      Some(target) => target,
      None => cdp
        .call("Target.createTarget", json!({"url": "about:blank"}))
        .await?
        .str("targetId")
        .ok_or_else(|| Error::internal("the browser opened no page"))?,
    };
    let page = cdp.attach(&target).await?;
    cdp.call_in(&page, "Page.enable", json!({})).await?;
    tracing::debug!("browser tab of {} opened in {}", ctx.info.id, exe.display());
    let inner = Inner {
      browser,
      cdp,
      page,
      at: None,
      closed: false,
    };
    Ok(Self {
      inner: Mutex::new(inner),
    })
  }

  /// End the browser, its cookies kept in the session first.
  async fn close(&self, ctx: &Ctx) {
    let mut inner = self.inner.lock().await;
    if inner.closed {
      return;
    }
    inner.closed = true;
    inner.sync(ctx).await;
    inner.cdp.close().await;
    inner.browser.wait().await;
  }
}

impl Inner {
  /// The cookies the site set in the browser, into the session.
  pub(super) async fn sync(&mut self, ctx: &Ctx) {
    match self.cdp.cookies(ctx.info.cookie_domains).await {
      Ok(cookies) => {
        for (name, value) in cookies {
          ctx.http.set_cookie(&name, &value);
        }
      }
      Err(e) => tracing::debug!("the browser's cookies were not read: {e}"),
    }
  }

  /// Start opening `url` in the page; its events tell when it has loaded.
  pub(super) async fn navigate(&mut self, url: &str) -> Result<()> {
    self.cdp.clear_events();
    let answer = self
      .cdp
      .call_in(&self.page, "Page.navigate", json!({"url": url}))
      .await?;
    if let Some(error) = answer.str("errorText") {
      return Err(Error::network(format!("{url}: {error}")));
    }
    self.at = Some(url.to_owned());
    Ok(())
  }

  /// Wait for the page's next load event; `false` when none comes before
  /// `deadline`.
  pub(super) async fn loaded(&mut self, deadline: Instant) -> Result<bool> {
    while let Some(event) = self.cdp.event(deadline).await? {
      if gone(&event, &self.page) {
        return Err(Error::internal("the browser's page closed"));
      }
      let ours = event.str("sessionId").as_deref() == Some(self.page.as_str());
      if ours && event.str("method").as_deref() == Some("Page.loadEventFired") {
        return Ok(true);
      }
    }
    Ok(false)
  }

  /// The value of `expression` evaluated in the page (a promise awaited),
  /// as JSON.
  pub(super) async fn evaluate(&mut self, expression: &str) -> Result<Value> {
    let params = json!({"expression": expression, "awaitPromise": true, "returnByValue": true});
    let answer = self
      .cdp
      .call_in(&self.page, "Runtime.evaluate", params)
      .await?;
    if let Some(details) = answer.get("exceptionDetails") {
      let text = details
        .first_str(&["exception.description", "text"])
        .unwrap_or_default();
      return Err(Error::upstream(format!(
        "a script in the page failed: {}",
        truncate(&one_line(&text), 200)
      )));
    }
    Ok(answer.at("result.value").clone())
  }
}

/// Whether `event` says the page (DevTools session `page`) is gone.
pub(super) fn gone(event: &Value, page: &str) -> bool {
  match event.str("method").as_deref() {
    Some("Target.detachedFromTarget") => event.str("params.sessionId").as_deref() == Some(page),
    Some("Inspector.detached" | "Inspector.targetCrashed") => {
      event.str("sessionId").as_deref() == Some(page)
    }
    _ => false,
  }
}
