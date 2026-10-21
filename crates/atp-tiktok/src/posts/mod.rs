//! Publishing and changing posts as TikTok Studio does it: videos and
//! photo posts from its upload page, their files sent through
//! [`uploads`](crate::uploads), then `/tiktok/web/project/post/v1/`
//! through the page itself ([`Browser`]); changes in [`edit`]. Nothing
//! goes out before the person has seen a summary and said yes.

mod body;
mod caption;
mod draft;
mod edit;

use std::path::Path;

use atp_core::output::note;
use atp_core::{Action, Draft, Error, ErrorCode, Result, Value, ValueExt};
use rand::Rng;

use self::caption::Caption;
use self::draft::{Kind, ask, check};
pub use self::edit::edit;

use crate::api::Api;
use crate::browser::{Browser, UPLOAD_PAGE};
use crate::playlists::lookup::{Me, Playlist};
use crate::refs;
use crate::uploads::{self, Uploads};

const POST: &str = "/tiktok/web/project/post/v1/";
/// A video longer than this (seconds) is a long one: no duets or stitches.
const LONG_VIDEO: f64 = 60.0;
/// Codes the post answers with: too many scheduled posts, posting too
/// often.
const SCHEDULED_MAX: i64 = 3_013_021;
const TOO_OFTEN: i64 = 2158;

/// What goes into the create request besides the files.
pub struct Post {
  pub caption: Caption,
  /// A photo post's title.
  pub title: String,
  /// 0 everyone, 1 only you, 2 friends (`visibility_type`).
  pub visibility: i64,
  /// When to publish, Unix seconds.
  pub schedule: Option<i64>,
  pub ai: bool,
  /// The playlist's id and name.
  pub playlist: Option<(String, String)>,
  /// A video over a minute.
  pub long: bool,
  /// The page's id of this post: 21 random characters.
  pub creation_id: String,
}

/// Publish a draft: check it, ask, upload its files, create the post.
pub async fn publish(api: &Api, draft: &Draft) -> Result<Action> {
  let kind = Kind::of(draft)?;
  check(draft, &kind)?;
  let topics = caption::topic_names(&draft.topics)?;
  let caption = Caption::new(&draft.text, &topics, &[]);
  caption.check()?;
  api.require_login()?;
  let playlist = match &draft.collection {
    Some(arg) => Some(playlist(api, arg).await?),
    None => None,
  };
  let mut post = Post {
    caption,
    title: draft.title.as_deref().unwrap_or_default().trim().to_owned(),
    visibility: visibility_code(draft.visibility.as_deref()),
    schedule: draft.schedule.map(|t| t.as_second() / 60 * 60),
    ai: draft.ai_generated,
    playlist: playlist.as_ref().map(|p| (p.id.clone(), p.name.clone())),
    long: matches!(kind, Kind::Video(path) if long(atp_core::file::video_duration(path))),
    creation_id: random_id(),
  };
  ask(draft, &kind, &post, playlist.as_ref())?;
  let app = api.app().await?;
  let me = app.user.ok_or_else(|| {
    Error::auth("TikTok did not recognize the session").with_hint(api.ctx.login_hint())
  })?;
  let uid = me.str("uid").unwrap_or_default();
  let browser = Browser::open(api, UPLOAD_PAGE).await?;
  let up = uploads::open(&browser, &uid).await?;
  let body = match kind {
    Kind::Video(path) => video(api, &up, &mut post, path).await?,
    Kind::Photos => photos(&up, &post, &draft.images).await?,
  };
  api.write_pause().await;
  let params = [
    ("app_name", "tiktok_web".to_owned()),
    ("channel", "tiktok_web".to_owned()),
    ("device_platform", "web".to_owned()),
    ("tz_name", tz_name()),
  ];
  let v = browser.post(POST, &params, &body).await?;
  let handle = me.first_str(&["uniqueId", "unique_id"]).unwrap_or_default();
  created(&v, &handle, matches!(kind, Kind::Photos))
}

async fn video(api: &Api, up: &Uploads<'_>, post: &mut Post, path: &Path) -> Result<Value> {
  note("uploading the video");
  let video = up.video(path).await?;
  note("waiting for TikTok to process the video");
  uploads::transcode(api, &video, path).await?;
  let duration_ms = uploads::duration_ms(&video, path);
  if duration_ms > 0 {
    post.long = long(Some(duration_ms as f64 / 1000.0));
  }
  Ok(body::video(post, &video, duration_ms))
}

async fn photos(up: &Uploads<'_>, post: &Post, paths: &[std::path::PathBuf]) -> Result<Value> {
  let mut images = Vec::new();
  for (i, path) in paths.iter().enumerate() {
    note(&format!("uploading photo {} of {}", i + 1, paths.len()));
    images.push(up.image(path).await?);
  }
  let ids: Vec<String> = images.iter().map(|_| random_id()).collect();
  Ok(body::photos(post, &images, &ids))
}

/// The new post's id and link; a post TikTok still works on has no id yet.
fn created(v: &Value, handle: &str, photo: bool) -> Result<Action> {
  let item = v.at("single_post_resp_list.0");
  for answer in [v, item] {
    let code = answer.i64("status_code").unwrap_or(0);
    if code != 0 {
      let msg = answer.str("status_msg").unwrap_or_default();
      return Err(refused(code, &msg));
    }
  }
  if let Some(id) = item.str("item_id").filter(|id| id != "0" && !id.is_empty()) {
    return Ok(
      Action::done("post", &id)
        .with_id(&id)
        .with_url(refs::video_url(handle, &id, photo)),
    );
  }
  let project = v.str("project_id").unwrap_or_default();
  Ok(Action::done("post", &project).with_message(
    "TikTok accepted the post and is still processing it; `atp tiktok studio` lists it once it is up",
  ))
}

/// A refused post as an error; the codes the page names, by name.
fn refused(code: i64, msg: &str) -> Error {
  match code {
    SCHEDULED_MAX => Error::input("TikTok takes at most 30 scheduled posts")
      .with_hint("wait until some of them are out"),
    TOO_OFTEN => Error::new(ErrorCode::RateLimited, "TikTok says you post too often")
      .with_hint("wait a while before posting again"),
    _ => crate::api::status_error(POST, code, msg),
  }
}

/// The account's playlist `arg` (an id, link or exact name): its id and
/// name.
async fn playlist(api: &Api, arg: &str) -> Result<Playlist> {
  Me::load(api).await?.playlist(api, arg).await
}

/// `visibility_type`: 0 everyone, 1 only you, 2 friends.
pub fn visibility_code(visibility: Option<&str>) -> i64 {
  match visibility {
    Some("private") => 1,
    Some("friends") => 2,
    _ => 0,
  }
}

pub fn visibility_name(code: i64) -> &'static str {
  match code {
    1 => "private",
    2 => "friends",
    _ => "public",
  }
}

fn long(seconds: Option<f64>) -> bool {
  seconds.is_some_and(|s| s > LONG_VIDEO)
}

/// The browser's time zone, as the page sends it (`tz_name`).
fn tz_name() -> String {
  jiff::tz::TimeZone::system()
    .iana_name()
    .unwrap_or("UTC")
    .to_owned()
}

/// A nanoid: 21 characters of the URL-safe alphabet.
fn random_id() -> String {
  const CHARS: &[u8] = b"useandom-26T198340PX75pxJACKVERYMINDBUSHWOLF_GQZbfghjklqvwyzrict";
  let mut rng = rand::rng();
  (0..21)
    .map(|_| char::from(CHARS[rng.random_range(0..CHARS.len())]))
    .collect()
}
