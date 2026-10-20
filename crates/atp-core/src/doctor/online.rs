//! Checks that go online: each saved session, and one request to a
//! well-known server for the network (through the proxy, when there is
//! one) and the clock.

use std::time::{Duration, Instant};

use jiff::{SignedDuration, Timestamp};

use super::{Check, Status};
use crate::app::{App, Entry};
use crate::cli::GlobalArgs;
use crate::http::{self, Http, HttpConfig};
use crate::model::Data;
use crate::store::Store;

/// A small page on a CDN that answers everywhere, mainland China included.
const PROBE: &str = "https://www.apple.com/library/test/success.html";

/// How far the clock may be off before signatures and logins suffer.
const SKEW: SignedDuration = SignedDuration::from_secs(30);

/// The `status` of every platform with a saved session, one after
/// another, each at its platform's pace.
pub(super) async fn sessions(app: &App, global: &GlobalArgs, offline: bool) -> Vec<Check> {
  let mut checks = Vec::new();
  for entry in app.platforms() {
    if !Store::new(entry.info.id).session_path().is_file() {
      continue;
    }
    let check = if offline {
      Check::new("session", Status::Skip, "--offline")
    } else {
      status(entry, global).await
    };
    checks.push(check.on(entry.info.id));
  }
  checks
}

/// `atp <platform> status`, without opening human checks.
async fn status(entry: &Entry, global: &GlobalArgs) -> Check {
  let matches = match (entry.command)().try_get_matches_from([entry.info.id, "status"]) {
    Ok(m) => m,
    Err(e) => return Check::new("session", Status::Fail, e.to_string()),
  };
  let mut global = global.clone();
  global.no_verify = true;
  match (entry.call)(global, matches).await {
    Ok(Data::Auth(auth)) if auth.authenticated => {
      let who = auth
        .user
        .map(|u| match u.handle {
          Some(handle) if handle != u.name => format!("{} (@{handle})", u.name),
          _ => u.name,
        })
        .unwrap_or_default();
      Check::new("session", Status::Ok, format!("logged in as {who}"))
    }
    Ok(Data::Auth(auth)) => Check::new(
      "session",
      Status::Warn,
      auth.message.unwrap_or_else(|| "not logged in".into()),
    ),
    Ok(_) => Check::new("session", Status::Ok, "checked"),
    Err(e) => Check::new(
      "session",
      Status::Fail,
      format!("{}: {}", e.code.as_str(), e.message),
    ),
  }
}

/// The proxy in effect and where it comes from (its password masked).
pub(super) fn proxy(global: &GlobalArgs) -> Check {
  let Some(url) = http::proxy_url(global.proxy.as_deref()) else {
    return Check::new("proxy", Status::Ok, "none: connections go out directly");
  };
  let from = if global.proxy.is_some() {
    "--proxy / ATP_PROXY"
  } else if env("ALL_PROXY") {
    "ALL_PROXY"
  } else {
    "HTTPS_PROXY"
  };
  let mut detail = format!("{} (from {from})", masked(&url));
  if env("NO_PROXY") {
    detail.push_str("; NO_PROXY lists hosts that bypass it");
  }
  Check::new("proxy", Status::Ok, detail)
}

/// Whether an environment variable is set, in either case.
fn env(name: &str) -> bool {
  [name.to_owned(), name.to_ascii_lowercase()]
    .iter()
    .any(|n| std::env::var_os(n).is_some_and(|v| !v.is_empty()))
}

/// `url` with the password of its credentials hidden.
fn masked(url: &str) -> String {
  match url::Url::parse(url) {
    Ok(mut u) if u.password().is_some() => {
      let _ = u.set_password(Some("***"));
      u.to_string()
    }
    _ => url.to_owned(),
  }
}

/// One request to [`PROBE`], through the proxy when there is one: is the
/// network there, and how far off is the clock (its `Date` header)?
pub(super) async fn network(global: &GlobalArgs, offline: bool) -> Vec<Check> {
  if offline {
    return vec![
      Check::new("network", Status::Skip, "--offline"),
      Check::new("clock", Status::Skip, "--offline"),
    ];
  }
  let host = url::Url::parse(PROBE)
    .ok()
    .and_then(|u| u.host_str().map(str::to_owned))
    .unwrap_or_default();
  let config = HttpConfig {
    proxy: global.proxy.clone(),
    timeout: Duration::from_secs(global.timeout.clamp(1, 15)),
    ..HttpConfig::default()
  };
  let http = match Http::new(&config, http::Cookies::new()) {
    Ok(http) => http,
    Err(e) => return vec![Check::new("network", Status::Fail, e.message)],
  };
  let sent = Timestamp::now();
  let started = Instant::now();
  let resp = match http.get(PROBE).no_cookies().retries(0).send().await {
    Ok(resp) => resp,
    Err(e) => {
      return vec![
        Check::new("network", Status::Fail, format!("{host}: {}", e.message)),
        Check::new("clock", Status::Skip, "no answer to compare with"),
      ];
    }
  };
  let took = started.elapsed();
  let via = if http::proxy_url(global.proxy.as_deref()).is_some() {
    "through the proxy"
  } else {
    "directly"
  };
  let network = Check::new(
    "network",
    Status::Ok,
    format!(
      "{host} answered HTTP {} in {} ms, {via}",
      resp.status.as_u16(),
      took.as_millis()
    ),
  );
  // The server's second is truncated: its midpoint is half a second on.
  let midway = sent + SignedDuration::try_from(took / 2).unwrap_or_default();
  vec![network, clock(resp.header("date"), midway, &host)]
}

/// How far this machine's clock is from the server's `Date` at `midway`.
fn clock(date: Option<&str>, midway: Timestamp, host: &str) -> Check {
  let Some(server) = date.and_then(|d| jiff::fmt::rfc2822::parse(d).ok()) else {
    return Check::new("clock", Status::Skip, format!("{host} sent no Date header"));
  };
  let server = server.timestamp() + SignedDuration::from_millis(500);
  let off = midway.duration_since(server);
  let secs = off.as_secs_f64().abs().round() as i64;
  let detail = match secs {
    0 => format!("in step with {host}'s clock"),
    _ if off.is_negative() => format!("{secs} s behind {host}'s clock"),
    _ => format!("{secs} s ahead of {host}'s clock"),
  };
  if off.abs() <= SKEW {
    Check::new("clock", Status::Ok, detail)
  } else {
    Check::new(
      "clock",
      Status::Warn,
      format!("{detail}: signed requests and logins may fail; set the time automatically (NTP)"),
    )
  }
}
