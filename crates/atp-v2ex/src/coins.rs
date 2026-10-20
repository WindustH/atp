//! Coins, through the site's pages with the login session (the API has
//! none of it): the balance, and the daily login reward (每日登录奖励).
//!
//! The reward page (`/mission/daily`) holds a button to
//! `/mission/daily/redeem?once=<n>` while today's reward waits; `once` is
//! the token V2EX puts in its forms. Redeeming answers with a redirect
//! back to the page, which then has no such button. Pages are read by
//! their links and the coin pictures, not their words, which follow the
//! account's language.

use std::sync::LazyLock;
use std::time::Duration;

use atp_core::{Action, Error, Result, Value, json};
use regex::Regex;

use crate::api::{Api, BASE};
use crate::parse;

const DAILY: &str = "/mission/daily";

static REDEEM: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"/mission/daily/redeem\?once=(\d+)").unwrap());
/// The reward button's label (`领取 12 铜币`), before its link.
static REWARD: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r#"value="[^"\d]*(\d+)[^"]*"[^>]*?/mission/daily/redeem"#).unwrap());

/// The coins of the logged-in account.
pub async fn balance(api: &Api) -> Result<Value> {
  api.require_session("balance")?;
  let html = api.html("/balance").await?;
  let [gold, silver, bronze] = parse::balance(&html)
    .ok_or_else(|| Error::upstream("the balance page shows no coins; V2EX may have changed it"))?;
  Ok(json!({
    "user": parse::page_member(&html),
    "gold": gold,
    "silver": silver,
    "bronze": bronze,
    "total_bronze": (gold * 100 + silver) * 100 + bronze,
  }))
}

/// Claim today's login reward, or say it was claimed already.
pub async fn check_in(api: &Api) -> Result<Action> {
  api.require_session("check-in")?;
  let html = api.html(DAILY).await?;
  let Some(token) = once(&html) else {
    return Ok(done(&html, "already checked in today"));
  };
  let reward = REWARD.captures(&html).map(|m| m[1].to_owned());
  api
    .ctx
    .http
    .pause(Duration::from_millis(500), Duration::from_millis(1500))
    .await;
  let referer = format!("{BASE}{DAILY}");
  api
    .page(&format!("{DAILY}/redeem?once={token}"), Some(&referer))
    .await?;
  let html = api.html(DAILY).await?;
  if once(&html).is_some() {
    return Err(Error::upstream(
      "V2EX did not hand out the reward: the button is still there",
    ));
  }
  let message = match reward {
    Some(coins) => format!("checked in, +{coins} bronze"),
    None => "checked in".to_owned(),
  };
  Ok(done(&html, &message))
}

/// The `once` of the redeem button, while today's reward waits.
fn once(html: &str) -> Option<String> {
  REDEEM.captures(html).map(|m| m[1].to_owned())
}

/// The check-in's Action, with the balance the page shows.
fn done(html: &str, message: &str) -> Action {
  let message = match parse::balance(html) {
    Some([gold, silver, bronze]) => {
      format!("{message}; balance {gold} gold, {silver} silver, {bronze} bronze")
    }
    None => message.to_owned(),
  };
  Action::done("check-in", "v2ex").with_message(message)
}
