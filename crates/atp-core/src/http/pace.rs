//! Pacing shared by every atp process using a platform: the start of the
//! latest request any of them reserved, in a small file under an advisory
//! lock (`<cache>/<platform>/pace`). A process that cannot take the lock
//! in time paces on its own.

use std::cell::RefCell;
use std::fs::{File, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The name of the file in a platform's cache folder.
pub const PACE_FILE: &str = "pace";

/// Tries to take the lock, a few milliseconds apart; others hold it only
/// to read and write a number.
const TRIES: u32 = 10;
const RETRY: Duration = Duration::from_millis(5);

/// A start further ahead than this is a clock that jumped, not a queue.
const STALE: Duration = Duration::from_secs(600);

/// Width of the number in the file (milliseconds since the epoch).
const WIDTH: usize = 20;

pub(super) struct Pace {
  path: PathBuf,
  /// Opened on first use, kept open.
  file: RefCell<Option<File>>,
}

impl Pace {
  pub fn new(path: PathBuf) -> Self {
    Self {
      path,
      file: RefCell::new(None),
    }
  }

  /// Reserve the first start at or after `earliest` that is `gap` after
  /// the latest start any process reserved, and record it. `None` when
  /// the file cannot be used: the caller paces on its own.
  pub async fn reserve(&self, earliest: SystemTime, gap: Duration) -> Option<SystemTime> {
    for _ in 0..TRIES {
      match self.try_reserve(earliest, gap) {
        Ok(Some(start)) => return Some(start),
        Ok(None) => tokio::time::sleep(RETRY).await,
        Err(e) => {
          tracing::debug!("pacing on its own: {}: {e}", self.path.display());
          return None;
        }
      }
    }
    tracing::debug!("pacing on its own: {} stays locked", self.path.display());
    None
  }

  /// One attempt: `None` while another process holds the lock.
  fn try_reserve(&self, earliest: SystemTime, gap: Duration) -> io::Result<Option<SystemTime>> {
    let mut slot = self.file.borrow_mut();
    let file = match slot.take() {
      Some(file) => file,
      None => open(&self.path)?,
    };
    let file = slot.insert(file);
    match file.try_lock() {
      Ok(()) => {}
      Err(TryLockError::WouldBlock) => return Ok(None),
      Err(TryLockError::Error(e)) => return Err(e),
    }
    let start = exchange(file, earliest, gap);
    let unlocked = file.unlock();
    let start = start?;
    unlocked?;
    Ok(Some(start))
  }
}

fn open(path: &std::path::Path) -> io::Result<File> {
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  let mut options = File::options();
  options.read(true).write(true).create(true).truncate(false);
  #[cfg(unix)]
  std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
  options.open(path)
}

/// Read the latest reserved start, write the next one.
fn exchange(file: &mut File, earliest: SystemTime, gap: Duration) -> io::Result<SystemTime> {
  let mut text = String::new();
  file.seek(SeekFrom::Start(0))?;
  file.read_to_string(&mut text)?;
  let latest = text
    .trim()
    .parse::<u64>()
    .ok()
    .map(|ms| UNIX_EPOCH + Duration::from_millis(ms))
    .filter(|at| *at <= SystemTime::now() + STALE);
  let start = match latest {
    Some(at) => earliest.max(at + gap),
    None => earliest,
  };
  let ms = start
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis();
  let line = format!("{ms:0WIDTH$}\n");
  file.seek(SeekFrom::Start(0))?;
  file.write_all(line.as_bytes())?;
  if text.len() != line.len() {
    file.set_len(line.len() as u64)?;
  }
  Ok(start)
}
