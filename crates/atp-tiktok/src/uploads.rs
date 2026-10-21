//! Files for posts, as TikTok Studio's upload page sends them: temporary
//! keys from `/api/v1/video/upload/auth/` (a path the page's scripts sign,
//! so asked through the [`Browser`]), then ByteDance's upload gateways
//! ([`atp_bytedance::upload`]) at `www.tiktok.com/top/v1`, signed for the
//! region the page picks for the account's data (its `gfdatav1` script):
//! videos to the VOD space `tiktok`, photos to the ImageX service
//! `photomode`. Once a video is in, Studio has it transcoded
//! (`transcode/enable`) and waits for that (`transcode/result`).

use std::path::Path;
use std::time::Duration;

use atp_bytedance::upload::{Gateway, Image, Keys, Uploader, Video};
use atp_core::task::{Progress, wait};
use atp_core::{Error, Result, Value, ValueExt, json};

use crate::api::{Api, WWW};
use crate::browser::{Browser, UPLOAD_PAGE};

const AUTH: &str = "/api/v1/video/upload/auth/";
const TRANSCODE: &str = "/api/v1/video/transcode/enable/";
const TRANSCODED: &str = "/api/v1/video/transcode/result/";
/// The gateway the page sends to in every data region.
const GATEWAY: &str = "https://www.tiktok.com/top/v1";
/// `business_tag` of a video for a post.
const VIDEO_TAG: &str = "tiktok_video_submission_web";
/// `transcode_status` of a finished and of a failed transcode.
const TRANSCODE_DONE: i64 = 3;
const TRANSCODE_FAILED: i64 = 4;
/// How long Studio's page waits for a transcode.
const TRANSCODE_WAIT: Duration = Duration::from_secs(3600);
/// The page's data region, as its `gfdatav1` script names it.
const REGION: &str = "(() => { try { \
   return JSON.parse(document.getElementById('gfdatav1').textContent).region || ''; \
   } catch (e) { return ''; } })()";

/// The uploaders of one post: videos and photos may come with keys of their own.
pub struct Uploads<'a> {
  pub video: Uploader<'a>,
  pub image: Uploader<'a>,
}

/// Keys for an upload by the account `user` (its uid), from the Studio
/// page in `browser`.
pub async fn open<'a>(browser: &Browser<'a>, user: &str) -> Result<Uploads<'a>> {
  let region = browser
    .eval(REGION)
    .await?
    .as_str()
    .unwrap_or("")
    .to_owned();
  let v = browser.get(AUTH, &[]).await?;
  let video = keys(&v, "video_token_v5")?;
  let image = keys(&v, "image_token_v5").unwrap_or_else(|_| video.clone());
  let gateway = gateway(&region);
  let ctx = &browser.api.ctx;
  Ok(Uploads {
    video: Uploader::new(ctx, gateway.clone(), video, user),
    image: Uploader::new(ctx, gateway, image, user),
  })
}

impl Uploads<'_> {
  pub async fn video(&self, path: &Path) -> Result<Video> {
    self.video.video(path).await
  }

  pub async fn image(&self, path: &Path) -> Result<Image> {
    self.image.image(path).await
  }
}

/// What the page sets for its upload SDK in `region`.
fn gateway(region: &str) -> Gateway {
  Gateway {
    site: "TikTok",
    origin: WWW.into(),
    referer: UPLOAD_PAGE.into(),
    vod: GATEWAY.into(),
    imagex: GATEWAY.into(),
    region: sign_region(region).into(),
    space: "tiktok".into(),
    image_service: "photomode".into(),
    params: Vec::new(),
    apply_params: vec![("device_platform".into(), "web".into())],
    video_params: vec![("business_tag".into(), VIDEO_TAG.into())],
    video_functions: json!([]),
    cookies: true,
  }
}

/// The region the gateway calls are signed for: US data (`ttp`), the
/// European and US-East-Red regions (`gcp`), Maliva, the rest (Singapore).
fn sign_region(region: &str) -> &'static str {
  match region {
    "ttp" | "ttp2" => "US-TTP",
    "ie" | "useastred" | "eu-ttp2" | "no1a" => "gcp",
    "maliva" => "us-east-1",
    _ => "ap-singapore-1",
  }
}

/// The keys of an upload-auth answer under `name`.
fn keys(v: &Value, name: &str) -> Result<Keys> {
  let token = v.at(name);
  let field = |key: &str| {
    token
      .str(key)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| Error::upstream(format!("TikTok handed out no upload keys ({name}.{key})")))
  };
  Ok(Keys::new(
    &field("access_key_id")?,
    // Sic: the page reads `secret_acess_key`.
    &field("secret_acess_key")?,
    &field("session_token")?,
    token.str("current_time").as_deref(),
  ))
}

/// Have the uploaded video transcoded and wait until it is, as the page
/// does before it lets a post go: `transcode/enable`, then
/// `transcode/result` until it says done.
pub async fn transcode(api: &Api, video: &Video, path: &Path) -> Result<()> {
  api
    .post(TRANSCODE, &[("video_id", video.vid.clone())])
    .await?;
  let body = json!({
    "scene": 0,
    "video_info": [{
      "file_key": video.vid,
      "video_id": video.vid,
      "original_width": video.meta.u64("Width").unwrap_or(0),
      "original_height": video.meta.u64("Height").unwrap_or(0),
      "original_duration_ms": duration_ms(video, path),
    }],
  });
  let page = format!("{WWW}/tiktokstudio/upload");
  wait(
    Duration::from_secs(2),
    Duration::from_secs(10),
    TRANSCODE_WAIT,
    async |_| {
      let v = api.post_json(TRANSCODED, &[], &body, &page).await?;
      let result = v
        .list("transcode_result")
        .iter()
        .find(|r| r.str("file_key").as_deref() == Some(video.vid.as_str()))
        .cloned()
        .unwrap_or(Value::Null);
      match result.i64("transcode_status") {
        Some(TRANSCODE_DONE) => Ok(Progress::Done(())),
        Some(TRANSCODE_FAILED) => Err(Error::upstream(format!(
          "TikTok could not process the video: {}",
          result
            .str("transcode_failed_reason")
            .unwrap_or_else(|| "no reason given".into())
        ))),
        _ => Ok(Progress::Running),
      }
    },
  )
  .await
}

/// The video's length in milliseconds: from VOD, else from the file
/// (MP4 and MOV), else 0.
pub fn duration_ms(video: &Video, path: &Path) -> u64 {
  let seconds = video
    .meta
    .f64("Duration")
    .filter(|d| *d > 0.0)
    .or_else(|| atp_core::file::video_duration(path));
  seconds.map_or(0, |s| (s * 1000.0).round() as u64)
}
