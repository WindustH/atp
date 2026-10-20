//! Files for the creator center, sent as its upload page sends them
//! (member.bilibili.com/york/videoup, bundles `index.*.js` and `607.*.js`).
//!
//! A video goes to UPOS: `preupload` (profile `ugcfx/bup`) names a host,
//! the file's place there (`upos_uri`), the key every call carries
//! (`X-Upos-Auth`) and the new file's `biz_id`, its cid; `?uploads` opens
//! a parts upload, each 10 MB part is a `PUT ?partNumber=…`, and the list
//! of parts with their ETags closes it. An archive refers to the file by
//! the name of `upos_uri` without its extension. A run that stopped
//! continues where it was. Covers go through `cover/up` as a data URL.

use std::path::Path;
use std::time::Duration;

use atp_core::file::Image;
use atp_core::http::{Method, Part as FormPart};
use atp_core::output::note;
use atp_core::upload::{LocalFile, Part, Parts, Resume};
use atp_core::{Error, ErrorCode, Result, Value, ValueExt, json};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::api::Api;

const PREUPLOAD: &str = "https://member.bilibili.com/preupload";
const COVER: &str = "https://member.bilibili.com/x/vu/web/cover/up";
const MEMBER: &str = "https://member.bilibili.com";
/// The upload page, the referer of its calls.
pub const PAGE: &str = "https://member.bilibili.com/platform/upload/video/frame";
const PROFILE: &str = "ugcfx/bup";
/// The uploader's version and build, as the page sends them.
const VERSION: &str = "2.14.0.0";
const BUILD: &str = "2140000";
/// The page's part size (`chunkSize`).
const PART_SIZE: u64 = 10 * 1024 * 1024;
/// How long a stopped upload may continue.
const RESUME_TTL: Duration = Duration::from_secs(3600);
/// The formats the page takes (`acceptExt`).
const FORMATS: &[&str] = &[
  "mp4", "flv", "avi", "wmv", "mov", "webm", "mpeg4", "ts", "mpg", "rm", "rmvb", "mkv", "m4v",
  "vob", "swf", "3gp", "mts", "m2v", "m2ts", "f4v", "m2t", "3g2", "asf",
];

/// An uploaded video: the name an archive refers to it by, and its cid.
#[derive(Debug, Clone)]
pub struct Uploaded {
  pub filename: String,
  pub cid: u64,
}

/// A parts upload on UPOS, kept between runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
  /// `https://HOST/PATH` of the file.
  url: String,
  auth: String,
  biz_id: u64,
  upload_id: String,
  filename: String,
}

/// Whether Bilibili takes a video in this format (by its extension).
pub fn check_format(path: &Path) -> Result<()> {
  let ext = path
    .extension()
    .and_then(|e| e.to_str())
    .map(str::to_ascii_lowercase)
    .unwrap_or_default();
  if FORMATS.contains(&ext.as_str()) {
    return Ok(());
  }
  Err(Error::input(format!(
    "Bilibili takes videos as {}; {} is not one",
    FORMATS.join(", "),
    path.display()
  )))
}

/// Upload a video file; a run that stopped continues where it was.
pub async fn video(api: &Api, path: &Path) -> Result<Uploaded> {
  api.require_login()?;
  check_format(path)?;
  let file = LocalFile::open(path).await?;
  let parts = Parts::new(&api.ctx, &file, "bilibili-upos");
  let mut state = match parts.resume::<Session>(RESUME_TTL) {
    Some(state) => state,
    None => Resume::new(open(api, &file).await?),
  };
  parts.keep(&state);
  let session = state.session.clone();
  let count = file.parts(PART_SIZE);
  note("uploading the video");
  let sent = parts
    .send(&mut state, PART_SIZE, async |part| {
      put(api, &session, &file, count, part).await
    })
    .await;
  if let Err(e) = sent {
    if !matches!(e.code, ErrorCode::NetworkError | ErrorCode::RateLimited) {
      parts.forget();
    }
    return Err(e);
  }
  let closed = close(api, &session, &file, &state.sent).await;
  parts.forget();
  closed?;
  Ok(Uploaded {
    filename: session.filename,
    cid: session.biz_id,
  })
}

/// `preupload`, then `?uploads`: where the file goes and its upload id.
async fn open(api: &Api, file: &LocalFile) -> Result<Session> {
  let resp = api
    .ctx
    .http
    .get(PREUPLOAD)
    .query("name", &file.name)
    .query("size", file.size)
    .query("r", "upos")
    .query("profile", PROFILE)
    .query("ssl", 0)
    .query("version", VERSION)
    .query("build", BUILD)
    .header("referer", PAGE)
    .header("origin", MEMBER)
    .send()
    .await?;
  let v = resp.value().unwrap_or(Value::Null);
  if !resp.status.is_success() || v.i64("OK") != Some(1) {
    return Err(preupload_error(resp.status.as_u16(), &v));
  }
  let (Some(endpoint), Some(uri), Some(auth), Some(biz_id)) = (
    v.str("endpoint"),
    v.str("upos_uri"),
    v.str("auth"),
    v.u64("biz_id"),
  ) else {
    return Err(Error::upstream(format!(
      "Bilibili's preupload answered without a place for the file: {}",
      atp_core::text::truncate(&v.to_string(), 200)
    )));
  };
  let path = uri.trim_start_matches("upos://");
  let url = format!("https:{endpoint}/{path}");
  let filename = path
    .rsplit('/')
    .next()
    .and_then(|name| name.split('.').next())
    .unwrap_or_default()
    .to_owned();
  let init = api
    .ctx
    .http
    .post(format!("{url}?uploads&output=json"))
    .query("profile", PROFILE)
    .query("filesize", file.size)
    .query("partsize", PART_SIZE)
    .query("biz_id", biz_id)
    .header("x-upos-auth", &auth)
    .header("origin", MEMBER)
    .header("referer", PAGE)
    .send()
    .await?
    .check()?
    .value()?;
  let upload_id = init
    .str("upload_id")
    .ok_or_else(|| Error::upstream("Bilibili's upload host gave no upload id"))?;
  Ok(Session {
    url,
    auth,
    biz_id,
    upload_id,
    filename,
  })
}

/// What a refused `preupload` means: too many uploads, or a human check.
fn preupload_error(status: u16, v: &Value) -> Error {
  let message = v
    .first_str(&["message", "msg"])
    .unwrap_or_else(|| format!("HTTP {status}"));
  match v.i64("code") {
    Some(406) => Error::new(ErrorCode::RateLimited, format!("Bilibili: {message}"))
      .with_hint("too many uploads for now; try again later"),
    Some(601) => Error::new(
      ErrorCode::VerificationRequired,
      format!("Bilibili wants a check before this upload: {message}"),
    )
    .with_url(PAGE)
    .unclearable(),
    _ if status == 401 || status == 403 => {
      Error::auth(format!("Bilibili refused the upload: {message}"))
        .with_hint("run `atp bili login`")
    }
    _ => Error::upstream(format!("Bilibili refused the upload: {message}")),
  }
}

/// One part; UPOS knows it by its ETag.
async fn put(api: &Api, s: &Session, file: &LocalFile, count: u32, part: &Part) -> Result<String> {
  let place = place(part.number, part.data.len() as u64, count, file.size);
  let resp = api
    .ctx
    .http
    .request(Method::PUT, &s.url)
    .query("partNumber", part.number)
    .query("uploadId", &s.upload_id)
    .queries(place)
    .header("x-upos-auth", &s.auth)
    .header("origin", MEMBER)
    .header("referer", PAGE)
    .bytes(part.data.clone(), "application/octet-stream")
    .send()
    .await?
    .check()?;
  let tag = resp.header("etag").unwrap_or("etag");
  Ok(tag.trim_matches('"').to_owned())
}

/// Where part `number` (from 1, `len` bytes) of `count` sits in a file of
/// `total` bytes: `chunk` counts from 0, `end` is exclusive.
fn place(number: u32, len: u64, count: u32, total: u64) -> [(&'static str, u64); 6] {
  let start = u64::from(number - 1) * PART_SIZE;
  [
    ("chunk", u64::from(number - 1)),
    ("chunks", u64::from(count)),
    ("size", len),
    ("start", start),
    ("end", start + len),
    ("total", total),
  ]
}

/// Close the upload with its parts.
async fn close(api: &Api, s: &Session, file: &LocalFile, tags: &[String]) -> Result<()> {
  let v = api
    .ctx
    .http
    .post(format!("{}?output=json", s.url))
    .query("name", &file.name)
    .query("profile", PROFILE)
    .query("uploadId", &s.upload_id)
    .query("biz_id", s.biz_id)
    .header("x-upos-auth", &s.auth)
    .header("origin", MEMBER)
    .header("referer", PAGE)
    .json(&parts(tags))
    .send()
    .await?
    .check()?
    .value()?;
  if v.i64("OK") == Some(1) {
    Ok(())
  } else {
    Err(Error::upstream(format!(
      "Bilibili did not take the uploaded parts: {}",
      atp_core::text::truncate(&v.to_string(), 200)
    )))
  }
}

/// The list of parts that closes an upload, with their ETags.
fn parts(tags: &[String]) -> Value {
  let parts: Vec<Value> = tags
    .iter()
    .enumerate()
    .map(|(i, tag)| json!({ "partNumber": i + 1, "eTag": tag }))
    .collect();
  json!({ "parts": parts })
}

/// Upload a cover (JPEG or PNG), the 16:9 one or the 4:3 one, `name`
/// in the progress note; its address.
pub async fn cover(api: &Api, path: &Path, name: &str, referer: &str) -> Result<String> {
  let image = Image::read(path).await?;
  if !matches!(image.mime, "image/jpeg" | "image/png") {
    return Err(Error::input("a Bilibili cover is a JPEG or PNG image"));
  }
  let data = base64::engine::general_purpose::STANDARD.encode(&image.data);
  let cover = format!("data:{};base64,{data}", image.mime);
  note(&format!("uploading the {name}"));
  let v = api
    .post(COVER)
    .multipart(vec![FormPart::text("cover", cover)])
    .header("referer", referer.to_owned())
    .header("origin", MEMBER)
    .send()
    .await?;
  v.str("url")
    .map(|url| https(&url))
    .ok_or_else(|| Error::upstream("the cover upload returned no address"))
}

/// A note on a 4:3 cover (首页推荐 4:3 封面) more than 15% off that
/// shape: the page crops a picked image to it in its cover editor, ATP
/// sends it as it is.
pub async fn cover43_note(path: &Path) -> Option<String> {
  let (width, height) = Image::read(path).await.ok()?.size();
  if width == 0 || height == 0 {
    return None;
  }
  let off = (width as f64 / height as f64) / (4.0 / 3.0);
  (!(0.85..=1.15).contains(&off)).then(|| {
    format!(
      "{} is {width}×{height}, far from 4:3; it goes uncropped (the web page crops it to 4:3)",
      path.display()
    )
  })
}

/// An address as the creator center's pages pass it on: their client
/// turns every `http://` it loads into `https://`.
pub fn https(url: &str) -> String {
  match url.strip_prefix("http://") {
    Some(rest) => format!("https://{rest}"),
    None => url.to_owned(),
  }
}
