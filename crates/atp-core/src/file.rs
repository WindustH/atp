//! Local files: format detection by magic bytes, and files to upload
//! ([`Upload`] for anything, [`Image`] where only images are accepted).

use std::path::Path;

use crate::error::{Error, Result};

/// File extension for the format the first bytes of a file reveal.
pub fn sniff(head: &[u8]) -> Option<&'static str> {
  let at = |range: std::ops::Range<usize>| head.get(range).unwrap_or_default();
  if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
    return Some("jpg");
  }
  if head.starts_with(b"\x89PNG") {
    return Some("png");
  }
  if head.starts_with(b"GIF8") {
    return Some("gif");
  }
  if head.starts_with(b"RIFF") && at(8..12) == b"WEBP" {
    return Some("webp");
  }
  if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
    return Some("webm");
  }
  if head.starts_with(b"FLV") {
    return Some("flv");
  }
  if at(4..8) == b"ftyp" {
    return Some(match at(8..12) {
      b"heic" | b"heix" | b"heim" | b"heis" | b"mif1" | b"msf1" => "heic",
      b"avif" | b"avis" => "avif",
      b"M4A " => "m4a",
      b"qt  " => "mov",
      _ => "mp4",
    });
  }
  None
}

/// An image read from disk, ready to upload.
#[derive(Debug, Clone)]
pub struct Image {
  pub data: Vec<u8>,
  /// `image/jpeg`, `image/png`, `image/gif`, `image/webp`, `image/heic` or `image/avif`.
  pub mime: &'static str,
  /// File name to send along, with an extension matching `mime`.
  pub name: String,
}

impl Image {
  /// Read `path` and detect its format from its content, not its extension.
  pub async fn read(path: &Path) -> Result<Self> {
    let data = tokio::fs::read(path)
      .await
      .map_err(|e| Error::input(format!("cannot read {}: {e}", path.display())))?;
    let ext = sniff(&data).unwrap_or_default();
    let mime = mime_for(ext);
    if !matches!(ext, "jpg" | "png" | "gif" | "webp" | "heic" | "avif") {
      return Err(Error::input(format!(
        "not a supported image: {}",
        path.display()
      )));
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    Ok(Self {
      data,
      mime,
      name: format!("{stem}.{ext}"),
    })
  }

  /// Width and height in pixels, `(0, 0)` when the header is unreadable.
  pub fn size(&self) -> (usize, usize) {
    imagesize::blob_size(&self.data)
      .map(|s| (s.width, s.height))
      .unwrap_or((0, 0))
  }

  /// Extension matching the detected format.
  pub fn ext(&self) -> &str {
    self.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("jpg")
  }
}

/// MIME type for a file extension (lower case, without the dot).
pub fn mime_for(ext: &str) -> &'static str {
  match ext {
    "jpg" | "jpeg" => "image/jpeg",
    "png" => "image/png",
    "gif" => "image/gif",
    "webp" => "image/webp",
    "heic" => "image/heic",
    "avif" => "image/avif",
    "bmp" => "image/bmp",
    "svg" => "image/svg+xml",
    "mp4" | "m4v" => "video/mp4",
    "mov" => "video/quicktime",
    "webm" => "video/webm",
    "mkv" => "video/x-matroska",
    "flv" => "video/x-flv",
    "mp3" => "audio/mpeg",
    "m4a" => "audio/mp4",
    "aac" => "audio/aac",
    "ogg" | "oga" | "opus" => "audio/ogg",
    "wav" => "audio/wav",
    "amr" => "audio/amr",
    "flac" => "audio/flac",
    "pdf" => "application/pdf",
    "zip" => "application/zip",
    "txt" | "md" | "log" => "text/plain",
    "csv" => "text/csv",
    "json" => "application/json",
    "html" | "htm" => "text/html",
    "doc" => "application/msword",
    "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "xls" => "application/vnd.ms-excel",
    "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "ppt" => "application/vnd.ms-powerpoint",
    "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    _ => "application/octet-stream",
  }
}

/// Any file read from disk, ready to upload: its bytes, MIME type (from the
/// content where it tells, else the extension) and file name.
#[derive(Debug, Clone)]
pub struct Upload {
  pub data: Vec<u8>,
  pub mime: &'static str,
  pub name: String,
}

impl Upload {
  pub async fn read(path: &Path) -> Result<Self> {
    let data = tokio::fs::read(path)
      .await
      .map_err(|e| Error::input(format!("cannot read {}: {e}", path.display())))?;
    let ext = path
      .extension()
      .and_then(|e| e.to_str())
      .map(str::to_ascii_lowercase)
      .unwrap_or_default();
    let mime = match sniff(&data) {
      Some(kind) => mime_for(kind),
      None => mime_for(&ext),
    };
    let name = path
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or("file")
      .to_owned();
    Ok(Self { data, mime, name })
  }

  /// What kind of media the file is, by its MIME type.
  pub fn kind(&self) -> crate::model::MediaKind {
    crate::model::MediaKind::from_mime(self.mime)
  }

  pub fn is_image(&self) -> bool {
    self.mime.starts_with("image/")
  }

  /// Width and height in pixels of an image, `None` for other files.
  pub fn size(&self) -> Option<(usize, usize)> {
    imagesize::blob_size(&self.data)
      .ok()
      .map(|s| (s.width, s.height))
  }
}

/// How long an MP4 or MOV video lasts, in seconds, from its movie header
/// (`moov/mvhd`), read without loading the file; `None` for other formats
/// or a header it cannot find.
pub fn video_duration(path: &Path) -> Option<f64> {
  use std::io::{Read, Seek, SeekFrom};
  let mut file = std::fs::File::open(path).ok()?;
  let end = file.metadata().ok()?.len();
  let moov = find_box(&mut file, 0, end, b"moov")?;
  let (start, size) = find_box(&mut file, moov.0, moov.0 + moov.1, b"mvhd")?;
  file.seek(SeekFrom::Start(start)).ok()?;
  let mut head = vec![0u8; size.min(32) as usize];
  file.read_exact(&mut head).ok()?;
  let num = |at: usize, len: usize| -> Option<u64> {
    let bytes = head.get(at..at + len)?;
    Some(bytes.iter().fold(0u64, |n, b| (n << 8) | u64::from(*b)))
  };
  // Version 1 holds 64-bit times: created, modified, timescale, duration.
  let (scale, duration) = match head.first()? {
    1 => (num(20, 4)?, num(24, 8)?),
    _ => (num(12, 4)?, num(16, 4)?),
  };
  (scale > 0).then(|| duration as f64 / scale as f64)
}

/// The content (offset, size) of the first box of `kind` between `from`
/// and `to`.
fn find_box(file: &mut std::fs::File, from: u64, to: u64, kind: &[u8; 4]) -> Option<(u64, u64)> {
  use std::io::{Read, Seek, SeekFrom};
  let mut at = from;
  while at + 8 <= to {
    file.seek(SeekFrom::Start(at)).ok()?;
    let mut head = [0u8; 16];
    file.read_exact(&mut head[..8]).ok()?;
    let mut size = u64::from(u32::from_be_bytes(head[..4].try_into().ok()?));
    let mut header = 8;
    if size == 1 {
      file.read_exact(&mut head[8..16]).ok()?;
      size = u64::from_be_bytes(head[8..16].try_into().ok()?);
      header = 16;
    } else if size == 0 {
      size = to - at;
    }
    if size < header {
      return None;
    }
    if &head[4..8] == kind {
      return Some((at + header, size - header));
    }
    at += size;
  }
  None
}
