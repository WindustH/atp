//! What a draft becomes on TikTok, what Studio's upload page refuses, and
//! the summary the person says yes to before anything is uploaded.

use std::path::Path;

use atp_core::cli::confirm;
use atp_core::output::note;
use atp_core::{Draft, Error, Result};

use super::Post;
use super::caption::utf16;
use crate::playlists::lookup::Playlist;

/// A photo post's title, at most (JavaScript string length).
pub const TITLE_MAX: usize = 90;
/// The photos of one post, at most, and their size.
const PHOTOS_MAX: usize = 35;
const PHOTO_BYTES_MAX: u64 = 50 << 20;
/// Scheduled posts go out 15 minutes to 10 days ahead (30 days for the
/// accounts in one of TikTok's experiments, so ATP lets TikTok decide
/// past 10).
const SCHEDULE_MIN: i64 = 15 * 60;
const SCHEDULE_MAX: i64 = 30 * 86_400;

/// What a draft publishes.
pub enum Kind<'a> {
  Video(&'a Path),
  Photos,
}

impl Kind<'_> {
  pub fn of(draft: &Draft) -> Result<Kind<'_>> {
    match (&draft.video, draft.images.is_empty()) {
      (Some(video), true) => Ok(Kind::Video(video)),
      (None, false) => Ok(Kind::Photos),
      (None, true) => Err(Error::input(
        "a TikTok post is a video (--video) or photos (-i); text-only posts are made in the app",
      )),
      (Some(_), false) => Err(Error::input("give either --video or -i")),
    }
  }

  pub fn name(&self) -> &'static str {
    match self {
      Kind::Video(_) => "video",
      Kind::Photos => "photo post",
    }
  }
}

/// What the upload page refuses, before anything is sent.
pub fn check(draft: &Draft, kind: &Kind) -> Result<()> {
  if draft.reply_to.is_some() || draft.quote.is_some() {
    return Err(Error::unsupported("post --reply-to / --quote"));
  }
  if draft.article.is_some() {
    return Err(Error::unsupported("post --article"));
  }
  if draft.cover.is_some() {
    return Err(
      Error::unsupported("post --cover")
        .with_hint("TikTok Studio takes a video's cover from its frames; ATP leaves the first one"),
    );
  }
  let title = utf16(draft.title.as_deref().unwrap_or_default().trim());
  match kind {
    Kind::Video(_) if title > 0 => Err(Error::input(
      "a TikTok video has no title; --title is for photo posts (the caption is the text)",
    )),
    Kind::Photos if title > TITLE_MAX => Err(Error::input(format!(
      "a photo post's title takes at most {TITLE_MAX} characters"
    ))),
    Kind::Photos => check_photos(&draft.images),
    Kind::Video(_) => Ok(()),
  }?;
  check_settings(draft)
}

fn check_photos(paths: &[std::path::PathBuf]) -> Result<()> {
  if paths.len() > PHOTOS_MAX {
    return Err(Error::input(format!(
      "a photo post takes at most {PHOTOS_MAX} photos"
    )));
  }
  for path in paths {
    let ext = path
      .extension()
      .and_then(|e| e.to_str())
      .map(str::to_ascii_lowercase)
      .unwrap_or_default();
    if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
      return Err(Error::input(format!(
        "{}: Studio takes JPEG, PNG and WebP photos",
        path.display()
      )));
    }
    if bytes(path) > PHOTO_BYTES_MAX {
      return Err(Error::input(format!(
        "{}: Studio takes photos up to 50 MB",
        path.display()
      )));
    }
  }
  Ok(())
}

fn check_settings(draft: &Draft) -> Result<()> {
  if draft.collection.is_some() && !matches!(draft.visibility.as_deref(), None | Some("public")) {
    return Err(Error::input("posts in a playlist must be public"));
  }
  if let Some(when) = draft.schedule {
    let ahead = when.as_second() - jiff::Timestamp::now().as_second();
    if !(SCHEDULE_MIN..=SCHEDULE_MAX).contains(&ahead) {
      return Err(Error::input(
        "--schedule must be 15 minutes to 10 days ahead (30 days on some accounts)",
      ));
    }
  }
  Ok(())
}

/// Show what goes out and ask (or take `--yes`).
pub fn ask(draft: &Draft, kind: &Kind, post: &Post, playlist: Option<&Playlist>) -> Result<()> {
  let caption = &post.caption;
  let mut lines = vec![format!("TikTok {}:", kind.name())];
  let mut line = |label: &str, value: String| lines.push(format!("  {label:<11} {value}"));
  if let Kind::Photos = kind {
    let title = draft.title.as_deref().map(str::trim).unwrap_or_default();
    line("title", shown(title));
  }
  line(
    "caption",
    shown(&atp_core::text::truncate(&caption.text, 60)),
  );
  let tags = caption.hashtags();
  if !tags.is_empty() {
    let tags: Vec<String> = tags.iter().map(|t| format!("#{t}")).collect();
    line("hashtags", tags.join(" "));
  }
  line(
    "visibility",
    draft.visibility.clone().unwrap_or_else(|| "public".into()),
  );
  line(
    "AI label",
    if draft.ai_generated {
      "yes (AI-generated)"
    } else {
      "no"
    }
    .into(),
  );
  if let Some(when) = draft.schedule {
    line("schedule", atp_core::text::fmt_time(when));
  }
  if let Some(list) = playlist {
    line("playlist", format!("{} ({})", list.name, list.id));
  }
  let files = draft.video.iter().chain(&draft.images);
  for path in files {
    line("file", format!("{} ({})", path.display(), size_of(path)));
  }
  line("settings", settings(kind, post).into());
  note(&lines.join("\n"));
  confirm(draft.yes, "publish this on TikTok")
}

/// Who may comment, duet and stitch: Studio's defaults, without duets and
/// stitches for videos over a minute (a length only some files tell before
/// the upload).
fn settings(kind: &Kind, post: &Post) -> &'static str {
  match kind {
    _ if post.long => {
      "comments allowed; duets and stitches off, as Studio sets them for videos over a minute"
    }
    Kind::Video(path) if atp_core::file::video_duration(path).is_none() => {
      "comments, duets and stitches allowed (duets and stitches off if the video is over a minute)"
    }
    _ => "comments, duets and stitches allowed, as Studio's defaults",
  }
}

fn shown(value: &str) -> String {
  if value.is_empty() {
    "(none)".into()
  } else {
    value.to_owned()
  }
}

fn bytes(path: &Path) -> u64 {
  std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn size_of(path: &Path) -> String {
  let bytes = bytes(path);
  if bytes >= 1 << 20 {
    format!("{:.1} MB", bytes as f64 / f64::from(1 << 20))
  } else {
    format!("{:.0} KB", (bytes as f64 / 1024.0).ceil())
  }
}
