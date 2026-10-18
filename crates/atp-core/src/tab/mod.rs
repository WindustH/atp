//! Platforms whose data only a real browser gets: an API behind a bot
//! check that runs the site's scripts, or a token the site's own pages
//! compute. ATP never computes or forges such tokens. A platform that
//! declares [`Platform::BROWSER_TAB`](crate::Platform::BROWSER_TAB) gets,
//! on its first use of [`Ctx::tab`], a Chromium-family browser of the
//! person's machine without a window, on the platform's own profile
//! ([`profile`]: the site's cookies, storage and device ids last between
//! runs), seeded with the session's cookies and through the same proxy as
//! ATP's requests. The site's own page loads in its one tab and its
//! scripts do what they do; then ATP
//!
//! - asks that page to make a request (`fetch` with the page's cookies,
//!   same origin: [`Tab::fetch`]), or
//! - reads the answer to a request the page makes itself
//!   ([`Tab::capture`], from the DevTools Network domain).
//!
//! The browser is used as it is: no other user agent, no hidden
//! automation marks, nothing added to pages. A page or answer that is a
//! human check (a captcha, a slider) is `verification_required`, which the
//! CLI opens in a window on the same profile for the person to pass; ATP
//! never clicks, drags or solves one. Requests through the tab keep the
//! platform's pacing, and the cookies the site sets come back into the
//! session. The browser ends with the command (the CLI closes it;
//! dropping the context ends it too).

mod answer;
mod network;
mod session;

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::MutexGuard;

pub use self::answer::Answer;
use self::answer::{check, check_error, login_page};
pub use self::network::Trigger;
use self::session::Inner;
pub use self::session::profile;
pub(crate) use self::session::{Session, Slot};
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::json::ValueExt;

/// How long a page may take to load.
const LOAD_WAIT: Duration = Duration::from_secs(30);
/// How long a page showing a check is given to pass it by itself (a
/// script that sets a cookie and reloads the page).
const CHECK_WAIT: Duration = Duration::from_secs(15);
/// How long a request made by the page may take.
const FETCH_WAIT: Duration = Duration::from_secs(30);
/// The largest answer read from the page.
const MAX_BODY: usize = 16 * 1024 * 1024;

/// What the page shows: its address and the start of its HTML.
const PROBE: &str = "({url: location.href, head: document.documentElement \
                     ? document.documentElement.outerHTML.slice(0, 16384) : ''})";

/// `fetch` in the page, with its cookies: status, final address, type and
/// text of the answer, or what went wrong.
const FETCH: &str = "async (url, init, wait, limit) => {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), wait);
  try {
    const resp = await fetch(url, Object.assign({credentials: 'include'}, init, {signal: ctrl.signal}));
    const body = await resp.text();
    if (body.length > limit) return {error: `the answer is larger than ${limit} characters`};
    return {status: resp.status, url: resp.url, type: resp.headers.get('content-type'), body};
  } catch (e) {
    return {error: String((e && e.message) || e)};
  } finally {
    clearTimeout(timer);
  }
}";

/// A platform's browser tab ([`Ctx::tab`]).
pub struct Tab<'a> {
  ctx: &'a Ctx,
  session: &'a Session,
}

impl<'a> Tab<'a> {
  pub(crate) fn new(ctx: &'a Ctx, session: &'a Session) -> Self {
    Self { ctx, session }
  }

  async fn lock(&self) -> Result<MutexGuard<'a, Inner>> {
    let inner = self.session.inner.lock().await;
    if inner.closed {
      return Err(Error::internal("the browser tab was closed"));
    }
    Ok(inner)
  }

  /// Open `url` and wait until it has loaded and a check it shows has
  /// passed by itself: the address the page shows then.
  pub async fn goto(&self, url: &str) -> Result<String> {
    let mut inner = self.lock().await?;
    let at = self.visit(&mut inner, url).await;
    inner.sync(self.ctx).await;
    at
  }

  /// `fetch(url, init)` made by a page of `page`'s site, with its cookies;
  /// the tab opens `page` first unless it shows a page of the same origin.
  /// `init` holds `fetch`'s options (`method`, `headers`, `body`). An
  /// answer that is a check makes the tab open `page` again, for its
  /// scripts to pass the check, and ask once more.
  pub async fn fetch(&self, page: &str, url: &str, init: Value) -> Result<Answer> {
    let mut inner = self.lock().await?;
    let answer = self.fetch_in(&mut inner, page, url, &init).await;
    inner.sync(self.ctx).await;
    answer
  }

  async fn fetch_in(
    &self,
    inner: &mut Inner,
    page: &str,
    url: &str,
    init: &Value,
  ) -> Result<Answer> {
    if !same_origin(inner.at.as_deref(), page) {
      self.visit(inner, page).await?;
    }
    let answer = self.request(inner, url, init).await?;
    if answer.check().is_none() {
      return Ok(answer);
    }
    self.visit(inner, page).await?;
    self.request(inner, url, init).await
  }

  /// The answer to the first request the page makes after `trigger` whose
  /// address `matches` accepts, read from the browser's network events.
  /// Answers that are checks are passed over while the page may still get
  /// past them by itself.
  pub async fn capture(
    &self,
    trigger: Trigger<'_>,
    matches: impl Fn(&str) -> bool,
  ) -> Result<Answer> {
    let mut inner = self.lock().await?;
    let answer = network::capture(self, &mut inner, trigger, &matches).await;
    inner.sync(self.ctx).await;
    answer
  }

  /// The value of a JavaScript expression evaluated in the page (a promise
  /// is awaited), as JSON.
  pub async fn eval(&self, expression: &str) -> Result<Value> {
    let mut inner = self.lock().await?;
    inner.evaluate(expression).await
  }

  /// Start opening `url` at the platform's pace.
  async fn navigate(&self, inner: &mut Inner, url: &str) -> Result<()> {
    self.ctx.http.throttle().await;
    let opened = inner.navigate(url).await;
    self.ctx.http.sent();
    opened
  }

  /// Open `url` at the platform's pace, then wait for it to settle.
  async fn visit(&self, inner: &mut Inner, url: &str) -> Result<String> {
    self.navigate(inner, url).await?;
    inner.loaded(Instant::now() + LOAD_WAIT).await?;
    self.settle(inner, url).await
  }

  /// The address a loaded page shows once a check it shows has passed by
  /// itself, or the check as an error; a login page in place of `url` is
  /// `not_authenticated`.
  async fn settle(&self, inner: &mut Inner, url: &str) -> Result<String> {
    let deadline = Instant::now() + CHECK_WAIT;
    loop {
      let probe = match inner.evaluate(PROBE).await {
        Ok(probe) => probe,
        // The page went on to another document meanwhile.
        Err(_) if inner.loaded(deadline).await? => continue,
        Err(e) => return Err(e),
      };
      let at = probe.str("url").unwrap_or_else(|| url.to_owned());
      inner.at = Some(at.clone());
      let Some(name) = probe.str("head").as_deref().and_then(check) else {
        if login_page(&at) && !login_page(url) {
          return Err(
            Error::auth(format!(
              "{} shows its login page for {url}",
              self.ctx.info.name
            ))
            .with_hint(self.ctx.login_hint()),
          );
        }
        return Ok(at);
      };
      if !inner.loaded(deadline).await? {
        return Err(check_error(self.ctx.info.name, name, url));
      }
    }
  }

  /// `fetch` in the page at the platform's pace.
  async fn request(&self, inner: &mut Inner, url: &str, init: &Value) -> Result<Answer> {
    let script = format!(
      "({FETCH})({}, {init}, {}, {MAX_BODY})",
      json!(url),
      FETCH_WAIT.as_millis()
    );
    self.ctx.http.throttle().await;
    let result = inner.evaluate(&script).await;
    self.ctx.http.sent();
    let v = result?;
    if let Some(error) = v.str("error") {
      return Err(Error::network(format!(
        "{}: {url}: {error}",
        self.ctx.info.name
      )));
    }
    Ok(self.answer(
      v.u64("status").unwrap_or_default() as u16,
      v.str("url").unwrap_or_else(|| url.to_owned()),
      v.str("type"),
      v.str("body").unwrap_or_default(),
      inner.at.clone().unwrap_or_default(),
    ))
  }

  fn answer(
    &self,
    status: u16,
    url: String,
    content_type: Option<String>,
    body: String,
    page: String,
  ) -> Answer {
    Answer {
      status,
      url,
      content_type,
      body,
      page,
      platform: self.ctx.info.name,
      login_hint: self.ctx.login_hint(),
    }
  }
}

/// Whether the page at `at` has the origin of `url`.
fn same_origin(at: Option<&str>, url: &str) -> bool {
  let origin = |u: &str| url::Url::parse(u).ok().map(|u| u.origin());
  matches!((at.and_then(origin), origin(url)), (Some(a), Some(b)) if a == b && a.is_tuple())
}
