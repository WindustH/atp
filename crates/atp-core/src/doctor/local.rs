//! Checks on this machine: ATP's folders, who may read the saved
//! sessions, ffmpeg and a Chromium-family browser. Nothing here goes
//! online or runs another program.

use std::path::Path;

use super::{Check, Status};
use crate::app::App;
use crate::chrome;
use crate::store::Store;

/// The config folder (sessions, data files) and the cache folder.
pub(super) fn dirs(config: &Path, cache: &Path) -> Vec<Check> {
  let home = std::env::var_os("ATP_HOME").map_or(String::new(), |_| " (ATP_HOME)".to_owned());
  let dir = |name: &'static str, path: &Path, holds: &str| {
    let made = if path.is_dir() { "" } else { ", not made yet" };
    Check::new(
      name,
      Status::Ok,
      format!("{}{home}: {holds}{made}", path.display()),
    )
  };
  vec![
    dir("config dir", config, "sessions and data files"),
    dir(
      "cache dir",
      cache,
      "caches, #N lists, browser profiles, pacing",
    ),
  ]
}

/// Each saved session file, and whether anyone but its owner may read it.
pub(super) fn session_files(app: &App) -> Vec<Check> {
  app
    .platforms()
    .iter()
    .filter_map(|e| {
      let path = Store::new(e.info.id).session_path();
      path.is_file().then(|| permissions(&path).on(e.info.id))
    })
    .collect()
}

#[cfg(unix)]
fn permissions(path: &Path) -> Check {
  use std::os::unix::fs::PermissionsExt;
  let mode = match std::fs::metadata(path) {
    Ok(meta) => meta.permissions().mode() & 0o777,
    Err(e) => {
      return Check::new(
        "session file",
        Status::Fail,
        format!("{}: {e}", path.display()),
      );
    }
  };
  if mode & 0o077 == 0 {
    return Check::new(
      "session file",
      Status::Ok,
      format!("{} (mode {mode:o})", path.display()),
    );
  }
  Check::new(
    "session file",
    Status::Warn,
    format!(
      "{} is open to group or others (mode {mode:o}); `chmod 600 {}`",
      path.display(),
      path.display()
    ),
  )
}

#[cfg(not(unix))]
fn permissions(path: &Path) -> Check {
  Check::new(
    "session file",
    Status::Ok,
    format!(
      "{} (permissions follow the user profile's folder)",
      path.display()
    ),
  )
}

/// ffmpeg on `PATH`, for merging tracks, audio only and speech segments.
pub(super) fn ffmpeg() -> Check {
  match chrome::on_path("ffmpeg") {
    Some(path) => Check::new("ffmpeg", Status::Ok, path.display().to_string()),
    None => Check::new(
      "ffmpeg",
      Status::Warn,
      "not on PATH: `download` keeps the video and audio of split videos apart, and \
       --audio-only / --split need it",
    ),
  }
}

/// The browser human checks and browser tabs use.
pub(super) fn browser(app: &App) -> Check {
  let tabs: Vec<&str> = app
    .platforms()
    .iter()
    .filter(|e| e.browser_tab)
    .map(|e| e.info.id)
    .collect();
  let Some(path) = chrome::find() else {
    return Check::new(
      "browser",
      Status::Warn,
      format!(
        "no Chrome, Chromium, Edge or Brave found (ATP_BROWSER names one): human checks \
         open in the default browser without coming back to ATP, and {} cannot be read",
        tabs.join(", ")
      ),
    );
  };
  let chosen = if std::env::var_os("ATP_BROWSER").is_some_and(|v| !v.is_empty()) {
    " (ATP_BROWSER)"
  } else {
    ""
  };
  let window = if chrome::display() {
    ""
  } else {
    "; no display, so browser tabs run without a window"
  };
  Check::new(
    "browser",
    Status::Ok,
    format!("{}{chosen}{window}", path.display()),
  )
}
