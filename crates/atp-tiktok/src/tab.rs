//! Writes TikTok's own page makes, in the browser tab ([`atp_core::tab`]):
//! the playlist calls, whose parameters go in a form body. ATP's `X-Bogus`
//! ([`crate::sign`]) signs a query over an empty body, so it would sign
//! these wrong; the page's signing script signs them as it signs the page's
//! own calls (`msToken`, `X-Bogus`, `X-Gnarly`). ATP adds only what the
//! page's request library adds: the device block, built in the page from
//! its app context and the browser, the `tt-csrf-token` of that context
//! where the page sends one, and the fixed header of its CSRF script.
//!
//! The page signs calls once its signing script runs, which it starts for
//! its own first API call: ATP waits for a signed request of the page
//! before it asks, and sends nothing when none comes.

use atp_core::tab::{Tab, Trigger};
use atp_core::{Error, ErrorCode, Result, Value, ValueExt, json};

use crate::api::{self, Api, WWW};

/// The device block as the page's request library builds it (its risk
/// parameters, `WebIdLastTime` and `odinId`), from the page's app context
/// and the browser, with the context's `csrfToken`; `null` when the page
/// carries no app context.
const DEVICE: &str = r#"(() => {
  let app = null;
  try {
    const data = document.getElementById('__UNIVERSAL_DATA_FOR_REHYDRATION__');
    app = JSON.parse(data.textContent).__DEFAULT_SCOPE__['webapp.app-context'];
  } catch (e) {}
  if (!app) return null;
  const ua = navigator.userAgent;
  const os = /Android|android|Adr/.test(ua) ? 'android' : /X11/.test(ua) ? 'linux'
    : /\(Macintosh; Intel /.test(ua) ? 'mac' : /Win\d{2}|Windows/.test(ua) ? 'windows' : 'unknown';
  const login = !!app.user;
  const params = {
    aid: '1988', app_name: 'tiktok_web', channel: 'tiktok_web', device_platform: 'web_pc',
    device_id: app.wid, region: app.region, priority_region: (app.user || {}).region || '',
    os, referer: document.referrer, cookie_enabled: navigator.cookieEnabled,
    screen_width: screen.width, screen_height: screen.height,
    browser_language: navigator.language, browser_platform: navigator.platform,
    browser_name: navigator.appCodeName, browser_version: navigator.appVersion,
    browser_online: navigator.onLine, app_language: app.language,
    webcast_language: app.language, tz_name: Intl.DateTimeFormat().resolvedOptions().timeZone,
    is_page_visible: document.visibilityState !== 'hidden', focus_state: document.hasFocus(),
    is_fullscreen: window.matchMedia('(display-mode: fullscreen)').matches,
    history_len: history.length, user_is_login: login, data_collection_enabled: login,
    WebIdLastTime: app.webIdCreatedTime, odinId: app.odinId,
  };
  const fp = (document.cookie.match(/s_v_web_id=(\w+)/) || [])[1];
  if (fp) params.verifyFp = fp;
  return {params, csrf: app.csrfToken || ''};
})()"#;

/// A form POST to `path` made by `page` (a www.tiktok.com page): its JSON
/// answer, whose status code the caller reads. `csrf` sends the
/// `tt-csrf-token`, as the page does for some of these calls only.
pub async fn post(
  api: &Api,
  page: &str,
  path: &str,
  form: &[(&str, String)],
  csrf: bool,
) -> Result<Value> {
  api.require_login()?;
  let tab = api.ctx.tab().await?;
  ready(&tab, page).await?;
  let device = tab.eval(DEVICE).await?;
  if device.is_null() {
    return Err(Error::upstream(format!(
      "TikTok's page {page} carried no app context; ATP sent nothing"
    )));
  }
  let mut headers = json!({"content-type": "application/x-www-form-urlencoded"});
  if csrf {
    headers["tt-csrf-token"] = json!(device.str("csrf").unwrap_or_default());
  }
  if api::CSRF_PROTECTED.contains(&path) {
    headers[api::CSRF_HEADER] = json!("1");
  }
  let body: Vec<String> = form
    .iter()
    .map(|(key, value)| format!("{key}={}", api::encode(value)))
    .collect();
  let url = format!("{WWW}{path}?{}", query(device.at("params")));
  let init = json!({"method": "POST", "headers": headers, "body": body.join("&")});
  api.write_pause().await;
  let answer = tab.fetch(page, &url, init).await?;
  if answer.body.trim().is_empty() && answer.status < 400 {
    return Err(
      Error::new(
        ErrorCode::VerificationRequired,
        format!("TikTok refused the request to {path} (empty answer)"),
      )
      .with_hint(api::WALLED_HINT),
    );
  }
  answer.json()
}

/// Open `page` and wait until it has made a signed API request of its
/// own: its signing script then signs what is asked from the page.
async fn ready(tab: &Tab<'_>, page: &str) -> Result<()> {
  let api_root = format!("{WWW}/api/");
  let signed = |url: &str| {
    url.starts_with(&api_root) && (url.contains("X-Bogus=") || url.contains("X-Gnarly="))
  };
  match tab.capture(Trigger::Goto(page), signed).await {
    Ok(_) => Ok(()),
    Err(e) if matches!(e.code, ErrorCode::UpstreamError) => Err(Error::upstream(format!(
      "TikTok's page {page} signed no request of its own in time, so nothing it would \
       sign was sent; retry later"
    ))),
    Err(e) => Err(e),
  }
}

/// The device block as the page's request library writes it: keys in
/// order, `encodeURIComponent` values, nothing for absent ones.
fn query(params: &Value) -> String {
  let Some(map) = params.as_object() else {
    return String::new();
  };
  let mut pairs: Vec<(&String, String)> = map
    .iter()
    .filter_map(|(key, value)| {
      let value = match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
      };
      Some((key, value))
    })
    .collect();
  pairs.sort_by(|a, b| a.0.cmp(b.0));
  pairs
    .iter()
    .map(|(key, value)| format!("{}={}", api::encode(key), api::encode(value)))
    .collect::<Vec<_>>()
    .join("&")
}
