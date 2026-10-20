//! What a request made in the tab answered, and the pages that stand in
//! for data: human checks (bot walls, captchas, sliders) and login walls.

use serde_json::Value;

use crate::error::{Error, ErrorCode, Result};
use crate::http::{StatusCode, status_error};
use crate::text::{one_line, truncate};

/// Marks of the pages bot checks put where a page or data was expected,
/// with the check's name. They are looked for near the start of a page,
/// where these checks put them.
const CHECKS: &[(&str, &str)] = &[
  ("aliyun_waf_", "Aliyun WAF"),
  ("aliyunwaf_", "Aliyun WAF"),
  ("_____tmd_____", "Alibaba's slider"),
  ("x5secdata", "Alibaba's slider"),
  ("cf-chl", "Cloudflare"),
  ("challenge-platform", "Cloudflare"),
  ("TEOCaptchaWidget", "Tencent EdgeOne"),
  ("EO_Bot_Ssid", "Tencent EdgeOne"),
  ("__jsl_clearance", "Jiasule"),
  // An empty page whose script (`/<path>/probe.js?v=…`) sets a cookie and
  // reloads; the pages it guards load `probev3.js` instead.
  ("/probe.js?v=", "a script check"),
  // A page made of Tencent's captcha alone, started as it loads.
  ("new TencentCaptcha(", "Tencent's captcha"),
];
/// Words in the titles of check pages.
const CHECK_TITLES: &[&str] = &[
  "验证码",
  "安全验证",
  "人机验证",
  "滑动验证",
  "Just a moment",
  "Attention Required",
  "Security Verification",
  "Captcha",
  "captcha",
];
/// How much of a page is searched for the marks of a check.
const CHECK_HEAD: usize = 16 * 1024;
/// Parts of the addresses of login pages.
const LOGIN_PATHS: &[&str] = &["login", "signin", "sign-in", "passport", "/sso"];

/// The check `html` is, if it is one: the name of the check.
pub(crate) fn check(html: &str) -> Option<&'static str> {
  let mut end = html.len().min(CHECK_HEAD);
  while !html.is_char_boundary(end) {
    end -= 1;
  }
  let head = &html[..end];
  if let Some((_, name)) = CHECKS.iter().find(|(mark, _)| head.contains(mark)) {
    return Some(name);
  }
  let title = head
    .split_once("<title")
    .and_then(|(_, rest)| rest.split_once('>'))
    .and_then(|(_, rest)| rest.split_once("</title>"))
    .map(|(title, _)| title)?;
  CHECK_TITLES
    .iter()
    .any(|word| title.contains(word))
    .then_some("a captcha")
}

/// Whether `url` is a login page.
pub(crate) fn login_page(url: &str) -> bool {
  let lower = url.to_ascii_lowercase();
  let Ok(parsed) = url::Url::parse(&lower) else {
    return false;
  };
  let place = format!("{}{}", parsed.host_str().unwrap_or(""), parsed.path());
  LOGIN_PATHS.iter().any(|part| place.contains(part))
}

/// What the hint of a check says when the tab wanted a window but found
/// no display.
pub const NO_DISPLAY: &str = "ATP's browser ran without a window, as no display was found \
                              (DISPLAY, WAYLAND_DISPLAY), and the site may turn such browsers \
                              away; run the command where a display is available";

/// The error for a human check of `platform` shown at `page`;
/// `no_display` when the tab wanted a window but found no display.
pub(crate) fn check_error(platform: &str, name: &str, page: &str, no_display: bool) -> Error {
  let hint = if no_display {
    NO_DISPLAY
  } else {
    "run the command on a terminal (without --no-verify): ATP opens the check in a window of \
     its browser for you to pass, then carries on"
  };
  Error::verification(format!(
    "{platform} shows a human check ({name}) instead of its data"
  ))
  .with_url(page)
  .with_hint(hint)
}

/// What a request made in the tab answered.
#[derive(Debug, Clone)]
pub struct Answer {
  pub status: u16,
  /// Where the answer came from, after redirects.
  pub url: String,
  pub content_type: Option<String>,
  pub body: String,
  /// The page of the tab the request was made from (where a check shows).
  pub page: String,
  pub(crate) platform: &'static str,
  pub(crate) login_hint: String,
  /// The tab wanted a window but found no display.
  pub(crate) no_display: bool,
}

impl Answer {
  /// The human check the body is, if it is one.
  pub fn check(&self) -> Option<&'static str> {
    let html = self.body.trim_start().starts_with('<')
      || self
        .content_type
        .as_deref()
        .is_some_and(|t| t.contains("html"));
    html.then(|| check(&self.body)).flatten()
  }

  /// `Ok` unless the answer is a human check (`verification_required`,
  /// with the tab's page to pass it on) or a login wall (a 401, a
  /// redirect to a login page: `not_authenticated`).
  pub fn passed(&self) -> Result<&Self> {
    if let Some(name) = self.check() {
      return Err(check_error(
        self.platform,
        name,
        &self.page,
        self.no_display,
      ));
    }
    if self.status == 401 || (login_page(&self.url) && !login_page(&self.page)) {
      return Err(
        Error::auth(format!("{} wants a login for this", self.platform))
          .with_hint(self.login_hint.clone()),
      );
    }
    Ok(self)
  }

  /// The body as JSON. A check, a login wall or any other page where data
  /// was expected is the error it stands for, never data.
  pub fn json(&self) -> Result<Value> {
    self.passed()?;
    if let Ok(v) = serde_json::from_str::<Value>(&self.body) {
      return Ok(v);
    }
    let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::BAD_GATEWAY);
    if !status.is_success() {
      return Err(status_error(status, &self.body));
    }
    Err(Error::new(
      ErrorCode::UpstreamError,
      format!(
        "{} answered with a page instead of data: {}",
        self.platform,
        truncate(&one_line(&self.body), 160)
      ),
    ))
  }
}
