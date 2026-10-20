//! Deals and articles as arguments: their links on the site and in the
//! app's mobile pages, text pasted from the app's share button with such
//! a link in it, a deal's number (`184316466`) or an article's id
//! (`ae6qdexk`).

use std::sync::LazyLock;

use atp_core::{Error, Result};
use regex::Regex;

use crate::api::WWW;

static LINK: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"https?://[A-Za-z0-9.-]*smzdm\.com/[^\s'\x22<>]*").unwrap());
static DEAL_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/p/(\d{4,12})(?:/|$)").unwrap());
static POST_PATH: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^/(talk/)?p/([0-9a-z]{6,12})(?:/|$)").unwrap());
static HASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-z]{6,12}$").unwrap());

/// What a page is.
#[derive(Debug, Clone)]
pub enum Target {
  /// 好价: www.smzdm.com/p/<number>/
  Deal(String),
  /// 原创, 资讯 …: post.smzdm.com/p/<id>/
  Article(String),
  /// 笔记: post.smzdm.com/talk/p/<id>/
  Note(String),
}

impl Target {
  /// The page's link, ending in `/`.
  pub fn url(&self) -> String {
    match self {
      Self::Deal(id) => format!("{WWW}/p/{id}/"),
      Self::Article(id) => format!("https://post.smzdm.com/p/{id}/"),
      Self::Note(id) => format!("https://post.smzdm.com/talk/p/{id}/"),
    }
  }
}

pub fn post(arg: &str) -> Result<Target> {
  let trimmed = arg.trim();
  if trimmed.len() <= 12 && !trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_digit()) {
    return Ok(Target::Deal(trimmed.to_owned()));
  }
  if HASH.is_match(trimmed) && trimmed.bytes().any(|b| b.is_ascii_lowercase()) {
    return Ok(Target::Article(trimmed.to_owned()));
  }
  from_link(trimmed).ok_or_else(|| {
    Error::input(format!(
      "not an SMZDM deal or article: {arg} (expected its link, a deal's number or an article's id)"
    ))
  })
}

fn from_link(text: &str) -> Option<Target> {
  let url = url::Url::parse(LINK.find(text)?.as_str()).ok()?;
  let host = url.host_str()?;
  let path = url.path();
  if host.starts_with("post.") {
    let caps = POST_PATH.captures(path)?;
    let id = caps[2].to_owned();
    return Some(if caps.get(1).is_some() {
      Target::Note(id)
    } else {
      Target::Article(id)
    });
  }
  if let Some(caps) = DEAL_PATH.captures(path) {
    return Some(Target::Deal(caps[1].to_owned()));
  }
  let caps = POST_PATH.captures(path)?;
  Some(Target::Article(caps[2].to_owned()))
}
