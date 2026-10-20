//! Publishing through the Graph API's content publishing, for business and
//! creator accounts: each post, and each item of a carousel, is first a
//! media container made from the public URL of its image or video
//! (`POST /<IG_USER_ID>/media`); Meta downloads the file itself. Once a
//! container is ready (`status_code` FINISHED), `media_publish` turns it
//! into a post. A single video is published as a reel.

use std::path::Path;
use std::time::Duration;

use atp_core::cli::confirm;
use atp_core::output::note;
use atp_core::text::{one_line, truncate};
use atp_core::{Action, Error, Result, ValueExt};
use atp_meta::Graph;
use atp_meta::graph::{Params, fields, params};
use url::Url;

use crate::api::Api;

/// Items a carousel holds at most.
const MAX_ITEMS: usize = 10;
/// Characters a caption holds at most.
const MAX_CAPTION: usize = 2200;
/// Checks of a container's status, five seconds apart: five minutes.
const STATUS_CHECKS: u32 = 60;

const HINT: &str = "publishing takes a Graph API token of a business or creator account: an \
   Instagram-login token with instagram_business_content_publish, or a Facebook token of the \
   linked Page with instagram_basic, instagram_content_publish and pages_read_engagement; run \
   `atp instagram login --token <token>` or set INSTAGRAM_ACCESS_TOKEN (INSTAGRAM_USER_ID picks \
   the account)";

/// What to publish. Media are public URLs: Meta downloads them itself.
#[derive(Debug, Default)]
pub struct Compose {
  pub caption: String,
  pub images: Vec<String>,
  pub videos: Vec<String>,
  /// The cover image of a reel.
  pub cover: Option<String>,
  /// Keep a reel off the profile grid (the Reels tab only).
  pub reels_only: bool,
}

impl Compose {
  /// `photo`, `reel` or `carousel`.
  fn kind(&self) -> &'static str {
    match (self.images.len(), self.videos.len()) {
      (1, 0) => "photo",
      (0, 1) => "reel",
      _ => "carousel",
    }
  }
}

/// Show what goes out, ask, then publish it.
pub async fn publish(api: &Api, compose: &Compose, yes: bool) -> Result<Action> {
  check(compose)?;
  let (graph, account) = api.graph_account("publishing", HINT).await?;
  let name = graph
    .get(&account, &fields("username"))
    .await
    .ok()
    .and_then(|v| v.str("username"))
    .map_or_else(|| account.clone(), |name| format!("@{name}"));
  ask(compose, &name, yes)?;
  let container = container(&graph, &account, compose).await?;
  ready(&graph, &container).await?;
  publish_container(&graph, &account, &container).await
}

/// Refuse what Instagram would: no media, too many, local files, a reel's
/// options on something else, a long caption.
fn check(compose: &Compose) -> Result<()> {
  let count = compose.images.len() + compose.videos.len();
  if count == 0 {
    return Err(Error::input(
      "nothing to publish: Instagram posts need --image URL or --video URL",
    ));
  }
  if count > MAX_ITEMS {
    return Err(Error::input(format!(
      "a carousel holds {MAX_ITEMS} images and videos at most"
    )));
  }
  let urls = compose.images.iter().chain(&compose.videos);
  for url in urls.chain(&compose.cover) {
    public(url)?;
  }
  if compose.kind() != "reel" && (compose.cover.is_some() || compose.reels_only) {
    return Err(Error::input(
      "--cover and --reels-only go with a single --video (a reel)",
    ));
  }
  if compose.caption.chars().count() > MAX_CAPTION {
    return Err(Error::input(format!(
      "the caption is longer than Instagram's {MAX_CAPTION} characters"
    )));
  }
  Ok(())
}

/// Fails unless `url` is an http(s) link: Meta downloads the media.
fn public(url: &str) -> Result<()> {
  let web = Url::parse(url)
    .ok()
    .is_some_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some());
  if web {
    return Ok(());
  }
  let what = if Path::new(url).exists() {
    "a local file"
  } else {
    "not a link"
  };
  Err(
    Error::input(format!(
      "{url} is {what}: Instagram's Graph API downloads media from public URLs"
    ))
    .with_hint(
      "put the file where anyone can download it (a web server, object storage with a public \
       link) and pass that https:// URL",
    ),
  )
}

/// Print what is about to be published, then ask (or take `--yes`).
fn ask(compose: &Compose, account: &str, yes: bool) -> Result<()> {
  let mut lines = vec![format!("Instagram {} on {account}:", compose.kind())];
  let mut line = |label: &str, value: &str| lines.push(format!("  {label:<8} {value}"));
  let caption = one_line(compose.caption.trim());
  if caption.is_empty() {
    line("caption", "(none)");
  } else {
    line("caption", &truncate(&caption, 60));
  }
  for url in &compose.images {
    line("image", url);
  }
  for url in &compose.videos {
    line("video", url);
  }
  if let Some(cover) = &compose.cover {
    line("cover", cover);
  }
  if compose.kind() == "reel" {
    let grid = if compose.reels_only {
      "Reels tab only"
    } else {
      "profile grid and Reels tab"
    };
    line("shown in", grid);
  }
  note(&lines.join("\n"));
  confirm(yes, "publish this on Instagram")
}

/// The container to publish: one photo, one reel, or a carousel.
async fn container(graph: &Graph<'_>, account: &str, compose: &Compose) -> Result<String> {
  let edge = format!("{account}/media");
  let caption = compose.caption.trim();
  match compose.kind() {
    "photo" => {
      let form = params(&[
        ("image_url", compose.images[0].as_str()),
        ("caption", caption),
      ]);
      create(graph, &edge, form).await
    }
    "reel" => {
      let feed = if compose.reels_only { "false" } else { "true" };
      let mut form = params(&[
        ("media_type", "REELS"),
        ("video_url", compose.videos[0].as_str()),
        ("caption", caption),
        ("share_to_feed", feed),
      ]);
      if let Some(cover) = &compose.cover {
        form.extend(params(&[("cover_url", cover.as_str())]));
      }
      create(graph, &edge, form).await
    }
    _ => carousel(graph, &edge, compose, caption).await,
  }
}

/// A container per item (images first, then videos), each ready before
/// the next, then the carousel holding them.
async fn carousel(
  graph: &Graph<'_>,
  edge: &str,
  compose: &Compose,
  caption: &str,
) -> Result<String> {
  let images = compose.images.iter().map(|url| ("IMAGE", url));
  let videos = compose.videos.iter().map(|url| ("VIDEO", url));
  let mut children = Vec::new();
  for (kind, url) in images.chain(videos) {
    let mut form = params(&[("is_carousel_item", "true")]);
    if kind == "VIDEO" {
      form.extend(params(&[
        ("media_type", "VIDEO"),
        ("video_url", url.as_str()),
      ]));
    } else {
      form.extend(params(&[("image_url", url.as_str())]));
    }
    let id = create(graph, edge, form).await?;
    ready(graph, &id).await?;
    children.push(id);
  }
  let children = children.join(",");
  let form = params(&[
    ("media_type", "CAROUSEL"),
    ("children", &children),
    ("caption", caption),
  ]);
  create(graph, edge, form).await
}

/// `POST /<IG_USER_ID>/media`: a container's id.
async fn create(graph: &Graph<'_>, edge: &str, form: Params) -> Result<String> {
  let v = graph.post(edge, &form).await?;
  v.str("id")
    .ok_or_else(|| Error::upstream("Instagram made no media container"))
}

/// Wait until Meta has downloaded and processed a container.
async fn ready(graph: &Graph<'_>, container: &str) -> Result<()> {
  for _ in 0..STATUS_CHECKS {
    let v = graph.get(container, &fields("status_code,status")).await?;
    match v.str("status_code").as_deref() {
      Some("FINISHED" | "PUBLISHED") | None => return Ok(()),
      Some("ERROR" | "EXPIRED") => {
        let why = v.str("status").unwrap_or_else(|| "no reason given".into());
        return Err(
          Error::upstream(format!("Instagram could not use the media: {why}")).with_hint(
            "the URLs must be public and lead to a JPEG image or an MP4 / MOV video that \
             Instagram accepts (see `atp guide instagram publish`)",
          ),
        );
      }
      _ => tokio::time::sleep(Duration::from_secs(5)).await,
    }
  }
  Err(Error::upstream(
    "the media were still processing after five minutes; nothing was published",
  ))
}

/// `media_publish`: the container becomes a post.
async fn publish_container(graph: &Graph<'_>, account: &str, container: &str) -> Result<Action> {
  let v = graph
    .post(
      &format!("{account}/media_publish"),
      &params(&[("creation_id", container)]),
    )
    .await?;
  let id = v
    .str("id")
    .ok_or_else(|| Error::upstream("Instagram published nothing"))?;
  let mut done = Action::done("publish", &id).with_id(&id);
  if let Ok(v) = graph.get(&id, &fields("permalink")).await
    && let Some(url) = v.str("permalink")
  {
    done = done.with_url(url);
  }
  Ok(done)
}
