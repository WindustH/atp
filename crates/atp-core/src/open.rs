//! Using a platform from Rust code. [`open`] gives one with the session the
//! command line keeps (`atp <id> login`, or `ATP_<ID>_COOKIE`); [`Open`]
//! takes cookies, a proxy or other settings in code instead. Every
//! operation of the command line is a [`Platform`] method, and a
//! platform's own commands go through [`Platform::run_extra`].
//!
//! The futures borrow the platform, which is not `Sync`: run them on a
//! current-thread runtime (`#[tokio::main(flavor = "current_thread")]`) or
//! in a `tokio::task::LocalSet`.
//!
//! A platform that reads through a browser tab
//! ([`Platform::BROWSER_TAB`]) keeps its browser from the first call that
//! needs it until the platform is dropped or
//! [`Ctx::close_tab`](crate::Ctx::close_tab) ends it.

use std::time::Duration;

use crate::account;
use crate::browser;
use crate::ctx::Ctx;
use crate::error::Result;
use crate::http::{Http, HttpConfig};
use crate::platform::Platform;
use crate::store::{Session, Store};
use crate::tab::Window;

/// The platform with the session the command line keeps for it; without
/// one, a guest (what works without an account still works).
pub fn open<P: Platform>() -> Result<P> {
  Open::new().platform()
}

/// Settings for opening a platform in code.
#[derive(Debug, Clone)]
pub struct Open {
  cookies: Option<String>,
  saved: bool,
  proxy: Option<String>,
  timeout: Duration,
  interval: Option<Duration>,
  site: Option<String>,
  window: Window,
}

impl Default for Open {
  fn default() -> Self {
    Self {
      cookies: None,
      saved: true,
      proxy: None,
      timeout: Duration::from_secs(30),
      interval: None,
      site: None,
      window: Window::default(),
    }
  }
}

impl Open {
  pub fn new() -> Self {
    Self::default()
  }

  /// Use these cookies (`name=value; ...`, a browser's `Cookie` header)
  /// instead of the saved session.
  pub fn cookies(mut self, header: impl Into<String>) -> Self {
    self.cookies = Some(header.into());
    self
  }

  /// Start without a session: as a guest, or to log in from code
  /// ([`Platform::login_token`], [`Platform::qr_start`]).
  pub fn guest(mut self) -> Self {
    self.saved = false;
    self
  }

  /// An `http`, `https` or `socks5` proxy URL.
  pub fn proxy(mut self, url: impl Into<String>) -> Self {
    self.proxy = Some(url.into());
    self
  }

  /// How long one request may take (30 seconds by default).
  pub fn timeout(mut self, timeout: Duration) -> Self {
    self.timeout = timeout;
    self
  }

  /// The least time between two requests; never below the platform's own
  /// pacing.
  pub fn interval(mut self, interval: Duration) -> Self {
    self.interval = Some(interval);
    self
  }

  /// The site of a platform reaching many sites (`--site` of `careers`:
  /// `bytedance`, or a career site's link).
  pub fn site(mut self, site: impl Into<String>) -> Self {
    self.site = Some(site.into());
    self
  }

  /// When a platform read through a browser opens it in a window
  /// (`--browser-window`; as the platform needs by default).
  pub fn browser_window(mut self, window: Window) -> Self {
    self.window = window;
    self
  }

  /// Open platform `P` with these settings.
  pub fn platform<P: Platform>(&self) -> Result<P> {
    let info = P::INFO;
    let store = Store::new(info.id);
    let session = match &self.cookies {
      Some(header) => Session {
        cookies: browser::parse_cookie_string(header),
        source: Some("code".into()),
        ..Session::default()
      },
      None if self.saved => account::load_session(&info, &store)?.session,
      None => Session::default(),
    };
    let config = HttpConfig {
      cookie_domains: info.cookie_domains,
      proxy: self.proxy.clone(),
      timeout: self.timeout,
      min_interval: self
        .interval
        .map_or(info.min_interval, |d| d.max(info.min_interval)),
    };
    let http = Http::new(&config, session.cookies)?;
    let mut ctx = Ctx::new(info, http, store, session.extra);
    ctx.site = self.site.clone();
    ctx.allow_tab(P::BROWSER_TAB, self.window.wanted(P::TAB_WINDOW));
    P::new(ctx)
  }
}

/// Keep the platform's session where the command line keeps it, when it
/// changed (a login from code, refreshed cookies or tokens), as the
/// command line does after each command.
pub fn save_session<P: Platform>(platform: &P) -> Result<()> {
  let ctx = platform.ctx();
  if !platform.logged_in() || !ctx.session_changed() {
    return Ok(());
  }
  let old = ctx.store.load_session()?.unwrap_or_default();
  let mut session = ctx.session();
  session.source = old.source.or_else(|| Some("code".into()));
  session.saved_at = Some(old.saved_at.unwrap_or_else(jiff::Timestamp::now));
  ctx.store.save_session(&session)
}
