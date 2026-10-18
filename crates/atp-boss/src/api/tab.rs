//! What goes through the browser tab ([`atp_core::tab`]): the search
//! page's own request for its list, job and company pages, and the API
//! calls the environment check turned away over plain HTTP, made again by
//! a page of the site with its cookies and the headers of its request
//! library.

use atp_core::tab::{Answer, Tab, Trigger};
use atp_core::{Error, ErrorCode, Result, Value, json};
use jiff::Timestamp;
use url::Url;

use super::{Api, Call, ENVIRONMENT, STALE_TOKEN, WWW, ZP_TOKEN, code, encode};

/// The requests the search page makes for its job list: the search, and
/// the recommendations it shows without search words.
const JOB_LISTS: &[&str] = &[
  "/wapi/zpgeek/search/joblist.json",
  "/wapi/zpgeek/pc/recommend/job/list.json",
];
/// The script reading the `token` of the page's data (`window._PAGE`).
const PAGE_TOKEN: &str = "(((window._PAGE || {}).token) || '').split('|')[0]";
/// The address the tab shows.
const WHERE: &str = "location.href";
/// The site's login page, where its security check sends a browser it
/// does not let through.
const LOGIN: &str = "/web/user/";
/// Marks of the site's security page and of the pages it sends the
/// browser back to.
const CHECKING: &[&str] = &["/security", "_security_check", "safe-validate", "/verify"];

/// The path of a link.
fn path(url: &str) -> Option<String> {
  Url::parse(url).ok().map(|u| u.path().to_owned())
}

impl Api {
  /// The platform's tab, noting whether it found a display for its window.
  async fn tab(&self) -> Result<Tab<'_>> {
    let tab = self.ctx.tab().await?;
    self.no_display.set(tab.no_display());
    Ok(tab)
  }

  /// The job list the search page `url` asks for when it opens. The
  /// first visit may meet the environment check (37) while the page's
  /// scripts set their cookie; the page is opened once more then.
  pub async fn jobs(&self, url: &str) -> Result<Value> {
    let tab = self.tab().await?;
    let mut v = self.list(&tab, url).await?;
    if code(&v) == Some(ENVIRONMENT) {
      v = self.list(&tab, url).await?;
    }
    self.data(v, url)
  }

  async fn list(&self, tab: &Tab<'_>, url: &str) -> Result<Value> {
    let matches = |u: &str| path(u).is_some_and(|p| JOB_LISTS.contains(&p.as_str()));
    let answer = match tab.capture(Trigger::Goto(url), matches).await {
      Ok(answer) => answer,
      Err(e) => return Err(self.missed(tab, e, url).await),
    };
    self.opened.set(true);
    answer.json()
  }

  /// Open a page of the site (a job, a company) and read it with
  /// `script`. The page's own document is waited for, past the site's
  /// security page if it shows first.
  pub async fn page(&self, url: &str, script: &str) -> Result<Value> {
    let tab = self.tab().await?;
    let wanted = path(url);
    let answer = match tab.capture(Trigger::Goto(url), |u| path(u) == wanted).await {
      Ok(answer) => answer,
      Err(e) => return Err(self.missed(&tab, e, url).await),
    };
    self.opened.set(true);
    document(&answer)?;
    tab.eval(script).await
  }

  /// What a page `url` that did not answer stands for. The site's
  /// security page, when its check fails, sends the browser on to the
  /// page again (`_security_check`), to itself, to the home page or to the
  /// login page: the tab showing one of those, or anything but `url`, is
  /// the check turning the browser away (`verification_required` with
  /// `url`); otherwise the error as it is.
  async fn missed(&self, tab: &Tab<'_>, error: Error, url: &str) -> Error {
    if !matches!(
      error.code,
      ErrorCode::UpstreamError | ErrorCode::NetworkError
    ) {
      return error;
    }
    let Ok(at) = tab.eval(WHERE).await else {
      return error;
    };
    let at = at.as_str().unwrap_or_default();
    let elsewhere = path(at) != path(url);
    if at.contains(LOGIN) {
      return self.turned_away(
        url,
        "the site's security check did not let ATP's browser through and sent it to the login \
         page",
      );
    }
    if elsewhere || CHECKING.iter().any(|mark| at.contains(mark)) {
      return self.turned_away(
        url,
        "the site's security check kept sending ATP's browser back instead of the page",
      );
    }
    error
  }

  /// A call made by the page `call.from` in the tab: a stale `zp_token`
  /// (122) is renewed and the environment check (37) given one more load
  /// of the page, each once.
  pub async fn in_tab(&self, call: &Call<'_>) -> Result<Value> {
    let tab = self.tab().await?;
    let mut v = self.send(&tab, call).await?;
    match code(&v) {
      Some(STALE_TOKEN) => {
        self.renew(&tab, call.from).await?;
        v = self.send(&tab, call).await?;
      }
      Some(ENVIRONMENT) => {
        tab.goto(call.from).await?;
        v = self.send(&tab, call).await?;
      }
      _ => {}
    }
    self.data(v, call.from)
  }

  async fn send(&self, tab: &Tab<'_>, call: &Call<'_>) -> Result<Value> {
    let mut headers = json!({
      "accept": "application/json, text/plain, */*",
      "x-requested-with": "XMLHttpRequest",
    });
    if let Some(token) = self.cookie(ZP_TOKEN) {
      headers["zp_token"] = json!(token);
    }
    if let Some(token) = self.page_token(tab, call.from).await? {
      headers["token"] = json!(token);
    }
    let mut query = call.query.to_vec();
    query.push(("_", Timestamp::now().as_millisecond().to_string()));
    let url = format!("{WWW}{}?{}", call.path, encode(&query));
    let mut init = json!({"method": "GET", "headers": headers});
    if let Some(form) = call.form {
      init["method"] = json!("POST");
      init["headers"]["content-type"] = json!("application/x-www-form-urlencoded");
      init["body"] = json!(encode(form));
    }
    tab.fetch(call.from, &url, init).await?.json()
  }

  /// The page's `token`, read once from the first page of the site the
  /// tab shows (opening `from` when it shows none yet).
  async fn page_token(&self, tab: &Tab<'_>, from: &str) -> Result<Option<String>> {
    if let Some(token) = self.token.borrow().as_ref() {
      return Ok(Some(token.clone()).filter(|t| !t.is_empty()));
    }
    if !self.opened.get() {
      tab.goto(from).await?;
      self.opened.set(true);
    }
    let token = tab
      .eval(PAGE_TOKEN)
      .await?
      .as_str()
      .unwrap_or_default()
      .to_owned();
    *self.token.borrow_mut() = Some(token.clone());
    Ok(Some(token).filter(|t| !t.is_empty()))
  }

  /// A new `zp_token` (the `bst` cookie), as the site's pages ask for one
  /// when the old one is refused.
  async fn renew(&self, tab: &Tab<'_>, from: &str) -> Result<()> {
    let init = json!({
      "method": "POST",
      "headers": {"accept": "application/json, text/plain, */*", "x-requested-with": "XMLHttpRequest"},
    });
    let url = format!("{WWW}/wapi/zppassport/set/zpToken");
    let v = tab.fetch(from, &url, init).await?.json()?;
    self.data(v, from).map(drop)
  }
}

/// `Ok` unless a page's document is a check or missing.
fn document(answer: &Answer) -> Result<()> {
  answer.passed()?;
  if answer.status == 404 {
    return Err(Error::not_found(format!(
      "BOSS Zhipin has no page {}",
      answer.url
    )));
  }
  Ok(())
}
