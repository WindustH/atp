//! Files for posts, as the creator page's upload SDK (`lib-byted-uploader`)
//! sends them: temporary keys from `/web/api/media/upload/auth/v5/`, then
//! ByteDance's upload gateways ([`atp_bytedance::upload`]): images through
//! ImageX, videos through VOD with `GetMeta` and a first-frame `Snapshot`
//! (the default cover) on commit; after a video, `video/enable/` tells the
//! creator platform the upload is done, as the page does.

use std::path::Path;

use atp_bytedance::upload::{self, Gateway, Keys};
use atp_core::{Error, Result, Value, ValueExt, json};

pub use atp_bytedance::upload::{Image, Video};

use crate::creator::{CREATOR, Creator};

const AUTH: &str = "/web/api/media/upload/auth/v5/";
const ENABLE: &str = "/web/api/media/video/enable/";
const VOD: &str = "https://vod.bytedanceapi.com/";
const IMAGEX: &str = "https://imagex.bytedanceapi.com/";
const REGION: &str = "cn-north-1";
const APP_ID: &str = "2906";
const SPACE: &str = "aweme";
const IMAGE_SERVICE: &str = "jm8ajry58r";

/// The upload gateways with one set of temporary keys, for `user` (the
/// account's uid, as the SDK's `userId`).
pub struct Uploader<'a> {
  creator: &'a Creator<'a>,
  inner: upload::Uploader<'a>,
}

impl<'a> Uploader<'a> {
  pub async fn new(creator: &'a Creator<'a>, user: &str) -> Result<Self> {
    let keys = sts(creator).await?;
    let gateway = Gateway {
      site: "Douyin",
      origin: CREATOR.into(),
      referer: format!("{CREATOR}/"),
      vod: VOD.into(),
      imagex: IMAGEX.into(),
      region: REGION.into(),
      space: SPACE.into(),
      image_service: IMAGE_SERVICE.into(),
      params: vec![
        ("app_id".into(), APP_ID.into()),
        ("user_id".into(), user.into()),
      ],
      apply_params: Vec::new(),
      video_params: Vec::new(),
      video_functions: json!([
        { "name": "GetMeta" },
        { "name": "Snapshot", "input": { "SnapshotTime": 0 } },
      ]),
      cookies: false,
    };
    let inner = upload::Uploader::new(&creator.api.ctx, gateway, keys, user);
    Ok(Self { creator, inner })
  }

  /// Upload an image (ImageX).
  pub async fn image(&self, path: &Path) -> Result<Image> {
    self.inner.image(path).await
  }

  /// Upload a video (VOD); a run that stopped continues where it was.
  pub async fn video(&self, path: &Path) -> Result<Video> {
    let video = self.inner.video(path).await?;
    // The page tells the creator platform the upload is done, and does not wait.
    let enable = self.creator.get(ENABLE).arg("video_id", &video.vid).send();
    if let Err(e) = enable.await {
      tracing::debug!("video/enable: {e}");
    }
    Ok(video)
  }
}

/// The temporary keys the creator platform hands out for uploads.
async fn sts(creator: &Creator<'_>) -> Result<Keys> {
  let v = creator.get(AUTH).send().await?;
  let auth: Value = v
    .str("auth")
    .and_then(|a| serde_json::from_str(&a).ok())
    .ok_or_else(|| Error::upstream("Douyin handed out no upload keys"))?;
  let field = |key: &str| {
    auth
      .str(key)
      .filter(|s| !s.is_empty())
      .ok_or_else(|| Error::upstream(format!("Douyin's upload keys lack {key}")))
  };
  Ok(Keys::new(
    &field("AccessKeyID")?,
    &field("SecretAccessKey")?,
    &field("SessionToken")?,
    auth.str("CurrentTime").as_deref(),
  ))
}
