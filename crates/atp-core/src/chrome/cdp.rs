//! The Chrome DevTools Protocol calls ATP makes: on the browser (cookies,
//! targets, closing) and, through a session attached to a page with
//! `flatten`, on one page (`call_in`). Events that arrive while a call
//! waits for its answer are kept for [`Cdp::event`] instead of dropped.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::browser::{Stored, pick};
use crate::error::{Error, Result};
use crate::http::Cookies;
use crate::json::ValueExt;
use crate::socket::Socket;

/// The most events kept between two reads of them; older ones are dropped.
const KEPT_EVENTS: usize = 4096;
/// How long one call may take (a page's `fetch` included).
const CALL_TIMEOUT: Duration = Duration::from_secs(90);

pub struct Cdp {
  socket: Socket,
  next: u64,
  events: VecDeque<Value>,
}

impl Cdp {
  pub async fn connect(endpoint: &str) -> Result<Self> {
    Ok(Self {
      socket: Socket::local(endpoint).await?,
      next: 0,
      events: VecDeque::new(),
    })
  }

  /// One command on the browser target.
  pub async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
    self.send(None, method, params).await
  }

  /// One command on the page attached as `session`.
  pub async fn call_in(&mut self, session: &str, method: &str, params: Value) -> Result<Value> {
    self.send(Some(session), method, params).await
  }

  async fn send(&mut self, session: Option<&str>, method: &str, params: Value) -> Result<Value> {
    self.next += 1;
    let id = self.next;
    let mut message = json!({"id": id, "method": method, "params": params});
    if let Some(session) = session {
      message["sessionId"] = json!(session);
    }
    self.socket.send_json(&message).await?;
    let answer = tokio::time::timeout(CALL_TIMEOUT, self.answer(id))
      .await
      .map_err(|_| Error::network(format!("the browser did not answer {method} in time")))??;
    if let Some(error) = answer.get("error") {
      return Err(Error::internal(format!("{method}: {error}")));
    }
    Ok(answer.at("result").clone())
  }

  /// The answer to call `id`, keeping the events read on the way.
  async fn answer(&mut self, id: u64) -> Result<Value> {
    loop {
      let message = self.read().await?;
      if message.u64("id") == Some(id) {
        return Ok(message);
      }
      if message.get("method").is_some() {
        if self.events.len() == KEPT_EVENTS {
          self.events.pop_front();
        }
        self.events.push_back(message);
      }
    }
  }

  async fn read(&mut self) -> Result<Value> {
    self
      .socket
      .next_json_lenient()
      .await?
      .ok_or_else(|| Error::internal("the browser closed its DevTools connection"))
  }

  /// The next event (`method`, `params`, `sessionId`), those kept while
  /// calls waited first; `None` once `deadline` has passed.
  pub async fn event(&mut self, deadline: Instant) -> Result<Option<Value>> {
    if let Some(event) = self.events.pop_front() {
      return Ok(Some(event));
    }
    loop {
      let read = tokio::time::timeout_at(deadline.into(), self.read()).await;
      let Ok(message) = read else {
        return Ok(None);
      };
      let message = message?;
      if message.get("method").is_some() {
        return Ok(Some(message));
      }
    }
  }

  /// Forget the events kept so far.
  pub fn clear_events(&mut self) {
    self.events.clear();
  }

  /// Set `cookies` on every domain in `domains` and its subdomains.
  pub async fn set_cookies(&mut self, cookies: &Cookies, domains: &[&str]) -> Result<()> {
    let list: Vec<Value> = domains
      .iter()
      .flat_map(|d| cookies.iter().map(move |(k, v)| cookie(k, v, d)))
      .collect();
    if !list.is_empty() {
      self
        .call("Storage.setCookies", json!({"cookies": list}))
        .await?;
    }
    Ok(())
  }

  /// Put the session's `cookies` into a kept profile: the ones it lacks,
  /// and the login's (`login`) whatever it holds. Cookies the site set
  /// in the profile since are newer than the session's copies.
  pub async fn seed(&mut self, cookies: &Cookies, domains: &[&str], login: &[&str]) -> Result<()> {
    let held = self.cookies(domains).await?;
    let wanted: Cookies = cookies
      .iter()
      .filter(|(name, _)| login.contains(&name.as_str()) || !held.contains_key(*name))
      .map(|(name, value)| (name.clone(), value.clone()))
      .collect();
    self.set_cookies(&wanted, domains).await
  }

  /// The browser's cookies on `domains`, one value per name.
  pub async fn cookies(&mut self, domains: &[&str]) -> Result<Cookies> {
    let answer = self.call("Storage.getCookies", json!({})).await?;
    let stored = answer.list("cookies").iter().filter_map(|c| {
      Some(Stored {
        name: c.str("name")?,
        value: c.str("value")?,
        domain: c.str("domain")?,
        expires: c
          .at("expires")
          .as_f64()
          .filter(|e| *e > 0.0)
          .map(|e| e as u64),
      })
    });
    Ok(pick(stored, domains))
  }

  /// Open `url` in a new tab and close the blank one the browser started with.
  pub async fn open(&mut self, url: &str) -> Result<()> {
    let blank = self.pages().await?;
    self
      .call("Target.createTarget", json!({"url": url}))
      .await?;
    for target in blank {
      let _ = self
        .call("Target.closeTarget", json!({"targetId": target}))
        .await;
    }
    Ok(())
  }

  /// The ids of the browser's pages.
  pub async fn pages(&mut self) -> Result<Vec<String>> {
    let targets = self.call("Target.getTargets", json!({})).await?;
    Ok(
      targets
        .list("targetInfos")
        .iter()
        .filter(|t| t.str("type").as_deref() == Some("page"))
        .filter_map(|t| t.str("targetId"))
        .collect(),
    )
  }

  /// Attach to page `target` for [`Cdp::call_in`]: the session's id.
  pub async fn attach(&mut self, target: &str) -> Result<String> {
    let answer = self
      .call(
        "Target.attachToTarget",
        json!({"targetId": target, "flatten": true}),
      )
      .await?;
    answer
      .str("sessionId")
      .ok_or_else(|| Error::internal("the browser gave no session for the page"))
  }

  pub async fn close(&mut self) {
    let _ = self.call("Browser.close", json!({})).await;
  }
}

/// A cookie for `domain` and its subdomains; `__Host-` cookies stay on the
/// host itself, as their prefix demands.
fn cookie(name: &str, value: &str, domain: &str) -> Value {
  if name.starts_with("__Host-") {
    return json!({
      "name": name, "value": value, "url": format!("https://{domain}/"),
      "path": "/", "secure": true,
    });
  }
  json!({
    "name": name, "value": value, "domain": format!(".{domain}"), "path": "/",
    "secure": name.starts_with("__Secure-"),
  })
}
