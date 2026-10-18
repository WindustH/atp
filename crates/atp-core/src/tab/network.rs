//! Answers to requests the page makes itself, read from the DevTools
//! Network domain: nothing is added to the page.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::session::gone;
use super::{Answer, Inner, MAX_BODY, Tab};
use crate::error::{Error, Result};
use crate::json::ValueExt;

/// How long the page is given to make the request and get its answer.
const CAPTURE_WAIT: Duration = Duration::from_secs(30);

/// What makes the page send the request [`Tab::capture`] waits for.
pub enum Trigger<'a> {
  /// Open this page (its own document may be what is waited for).
  Goto(&'a str),
  /// Run this script in the page shown: a call of the page's own code.
  Eval(&'a str),
}

pub(super) async fn capture(
  tab: &Tab<'_>,
  inner: &mut Inner,
  trigger: Trigger<'_>,
  matches: &dyn Fn(&str) -> bool,
) -> Result<Answer> {
  let page = inner.page.clone();
  let buffers = json!({"maxResourceBufferSize": MAX_BODY, "maxTotalBufferSize": MAX_BODY * 4});
  inner.cdp.call_in(&page, "Network.enable", buffers).await?;
  inner.cdp.clear_events();
  let pulled = match trigger {
    Trigger::Goto(url) => tab.navigate(inner, url).await,
    Trigger::Eval(script) => inner.evaluate(script).await.map(drop),
  };
  let answer = match pulled {
    Ok(()) => Watch::default().run(tab, inner, matches).await,
    Err(e) => Err(e),
  };
  let _ = inner.cdp.call_in(&page, "Network.disable", json!({})).await;
  answer
}

/// The requests waited for, and what became of them.
#[derive(Default)]
struct Watch {
  /// The responses of matching requests, by request id, until their
  /// bodies have come.
  wanted: HashMap<String, Value>,
  /// The last matching answer that was a check.
  check: Option<Answer>,
  /// Why the last matching request failed.
  failed: Option<String>,
}

impl Watch {
  /// Read the page's network events until a wanted answer has come whole.
  async fn run(
    mut self,
    tab: &Tab<'_>,
    inner: &mut Inner,
    matches: &dyn Fn(&str) -> bool,
  ) -> Result<Answer> {
    let page = inner.page.clone();
    let deadline = Instant::now() + CAPTURE_WAIT;
    while let Some(event) = inner.cdp.event(deadline).await? {
      if gone(&event, &page) {
        return Err(Error::internal("the browser's page closed"));
      }
      if event.str("sessionId").as_deref() != Some(page.as_str()) {
        continue;
      }
      let params = event.at("params");
      let id = params.str("requestId").unwrap_or_default();
      match event.str("method").as_deref() {
        Some("Network.responseReceived") => {
          let response = params.at("response");
          if response.str("url").is_some_and(|url| matches(&url)) {
            self.wanted.insert(id, response.clone());
          }
        }
        Some("Network.loadingFinished") => {
          let Some(response) = self.wanted.remove(&id) else {
            continue;
          };
          let answer = finished(tab, inner, &id, &response, params).await?;
          if answer.check().is_none() {
            return Ok(answer);
          }
          self.check = Some(answer);
        }
        Some("Network.loadingFailed")
          if self.wanted.remove(&id).is_some() && params.bool("canceled") != Some(true) =>
        {
          self.failed = params.str("errorText");
        }
        _ => {}
      }
    }
    self.missed(tab)
  }

  /// The error when no wanted answer came in time: the check the page
  /// stayed on, the request's failure, or none at all.
  fn missed(self, tab: &Tab<'_>) -> Result<Answer> {
    if let Some(answer) = self.check {
      answer.passed()?;
    }
    if let Some(error) = self.failed {
      return Err(Error::network(format!(
        "{}: the page's request failed: {error}",
        tab.ctx.info.name
      )));
    }
    Err(Error::upstream(format!(
      "{}'s page made no request ATP waits for within {} s",
      tab.ctx.info.name,
      CAPTURE_WAIT.as_secs()
    )))
  }
}

/// The answer of request `id`, whose `response` came before and whose
/// body has now come whole (`params` of `loadingFinished`).
async fn finished(
  tab: &Tab<'_>,
  inner: &mut Inner,
  id: &str,
  response: &Value,
  params: &Value,
) -> Result<Answer> {
  let url = response.str("url").unwrap_or_default();
  if params.u64("encodedDataLength").unwrap_or_default() > MAX_BODY as u64 {
    return Err(Error::upstream(format!(
      "the answer from {url} is larger than ATP reads"
    )));
  }
  let v = inner
    .cdp
    .call_in(
      &inner.page,
      "Network.getResponseBody",
      json!({"requestId": id}),
    )
    .await?;
  let mut body = v.str("body").unwrap_or_default();
  if v.bool("base64Encoded") == Some(true) {
    let bytes = STANDARD
      .decode(&body)
      .map_err(|e| Error::upstream(format!("an answer the browser read is not base64: {e}")))?;
    body = String::from_utf8_lossy(&bytes).into_owned();
  }
  let status = response.u64("status").unwrap_or_default() as u16;
  let shown = inner.at.clone().unwrap_or_default();
  Ok(tab.answer(status, url, response.str("mimeType"), body, shown))
}
