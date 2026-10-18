//! Chromium-family browsers driven over DevTools: finding one, starting it
//! on a profile with DevTools on a local port, and the protocol ([`cdp`]).
//! Human checks ([`crate::verify`]) open a page in a window on a throwaway
//! profile; platforms that read through a browser ([`crate::tab`]) run one
//! on a profile of their own, without a window or, where the site turns
//! such browsers away, in a small one.

pub mod cdp;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::output;

/// Executables looked up on `PATH` (Linux, BSD).
const COMMANDS: &[&str] = &[
  "google-chrome",
  "google-chrome-stable",
  "chromium",
  "chromium-browser",
  "microsoft-edge",
  "microsoft-edge-stable",
  "brave-browser",
  "brave",
];

/// App bundles under `/Applications` and `~/Applications` (macOS).
const BUNDLES: &[&str] = &[
  "Google Chrome.app/Contents/MacOS/Google Chrome",
  "Chromium.app/Contents/MacOS/Chromium",
  "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "Brave Browser.app/Contents/MacOS/Brave Browser",
];

/// Installs under `%ProgramFiles%`, `%ProgramFiles(x86)%` and
/// `%LocalAppData%` (Windows).
const INSTALLS: &[&str] = &[
  r"Google\Chrome\Application\chrome.exe",
  r"Microsoft\Edge\Application\msedge.exe",
  r"BraveSoftware\Brave-Browser\Application\brave.exe",
  r"Chromium\Application\chrome.exe",
];

/// The file a browser started with `--remote-debugging-port=0` writes its
/// port and DevTools path to.
const PORT_FILE: &str = "DevToolsActivePort";
/// The file ATP locks while a kept profile is in use.
const LOCK_FILE: &str = "atp.lock";

/// Whether a window can open: on Linux and the BSDs, an X or Wayland
/// display (`DISPLAY`, `WAYLAND_DISPLAY`; none over plain ssh); macOS and
/// Windows always have one.
pub fn display() -> bool {
  if cfg!(any(target_os = "macos", windows)) {
    return true;
  }
  ["DISPLAY", "WAYLAND_DISPLAY"]
    .iter()
    .any(|var| std::env::var_os(var).is_some_and(|v| !v.is_empty()))
}

/// The browser to drive: `ATP_BROWSER` (a path or a command), else the
/// first installed Chrome, Chromium, Edge or Brave.
pub fn find() -> Option<PathBuf> {
  if let Some(chosen) = std::env::var_os("ATP_BROWSER").filter(|v| !v.is_empty()) {
    let path = PathBuf::from(&chosen);
    return if path.is_file() {
      Some(path)
    } else {
      on_path(&chosen.to_string_lossy())
    };
  }
  candidates().into_iter().find(|p| p.is_file())
}

fn candidates() -> Vec<PathBuf> {
  if cfg!(target_os = "macos") {
    let mut roots = vec![PathBuf::from("/Applications")];
    roots.extend(dirs::home_dir().map(|h| h.join("Applications")));
    return roots
      .iter()
      .flat_map(|root| BUNDLES.iter().map(move |b| root.join(b)))
      .collect();
  }
  if cfg!(windows) {
    return ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"]
      .iter()
      .filter_map(std::env::var_os)
      .flat_map(|root| INSTALLS.iter().map(move |i| Path::new(&root).join(i)))
      .collect();
  }
  COMMANDS.iter().filter_map(|c| on_path(c)).collect()
}

/// `name` resolved against `PATH` (with `.exe` on Windows).
fn on_path(name: &str) -> Option<PathBuf> {
  let file = if cfg!(windows) && !name.ends_with(".exe") {
    format!("{name}.exe")
  } else {
    name.to_owned()
  };
  std::env::split_paths(&std::env::var_os("PATH")?)
    .map(|dir| dir.join(&file))
    .find(|p| p.is_file())
}

/// Where a browser keeps its profile.
pub enum Profile {
  /// An empty one, deleted when the browser ends.
  Throwaway,
  /// A platform's own, kept between runs (the site's cookies, storage and
  /// device ids), used by one browser at a time.
  Kept(PathBuf),
}

/// How to start a browser.
pub struct Launch<'a> {
  pub profile: Profile,
  /// Without a window (`--headless=new`).
  pub headless: bool,
  /// The size of the window, also the page's size without one.
  pub size: (u32, u32),
  /// Chrome's `--proxy-server`: `scheme://host:port` without credentials.
  pub proxy: Option<&'a str>,
}

/// A running browser. Dropping it ends the process: a kept profile is
/// given a few seconds to be written out, a throwaway one is deleted.
pub struct Browser {
  child: Child,
  profile: PathBuf,
  throwaway: bool,
  /// Held while a kept profile is in use.
  _lock: Option<File>,
}

impl Browser {
  /// Start `exe` on `launch.profile`; a kept profile another atp is using
  /// gives way to a throwaway one for this run.
  pub fn launch(exe: &Path, launch: Launch<'_>) -> Result<Self> {
    let (profile, lock) = match launch.profile {
      Profile::Kept(dir) => match lock(&dir)? {
        Some(lock) => (dir, Some(lock)),
        None => {
          output::note(&format!(
            "the browser profile {} is in use by another atp; this run starts from an empty one",
            dir.display()
          ));
          (throwaway()?, None)
        }
      },
      Profile::Throwaway => (throwaway()?, None),
    };
    // A port file left by an earlier run would name a closed port.
    let _ = std::fs::remove_file(profile.join(PORT_FILE));
    let mut cmd = Command::new(exe);
    cmd
      .arg(format!("--user-data-dir={}", profile.display()))
      .args([
        "--remote-debugging-port=0",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-sync",
        // The profile's cookies are encrypted with Chrome's fixed key
        // instead of one from the system's keyring or keychain, which is
        // neither read nor asked for.
        "--password-store=basic",
        "--use-mock-keychain",
      ])
      .arg(format!("--window-size={},{}", launch.size.0, launch.size.1));
    if launch.headless {
      cmd.arg("--headless=new");
    }
    if let Some(proxy) = launch.proxy {
      cmd.arg(format!("--proxy-server={proxy}"));
    }
    let child = cmd
      .arg("about:blank")
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .map_err(|e| Error::internal(format!("cannot start {}: {e}", exe.display())))?;
    Ok(Self {
      child,
      throwaway: lock.is_none(),
      profile,
      _lock: lock,
    })
  }

  /// The DevTools websocket of the browser, once it has written its port.
  pub async fn endpoint(&mut self) -> Result<String> {
    let file = self.profile.join(PORT_FILE);
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
      if let Ok(text) = std::fs::read_to_string(&file) {
        let mut lines = text.lines();
        if let (Some(port), Some(path)) = (lines.next(), lines.next()) {
          return Ok(format!("ws://127.0.0.1:{}{}", port.trim(), path.trim()));
        }
      }
      if self.exited() {
        break;
      }
      tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(Error::internal(
      "the browser did not open its DevTools port",
    ))
  }

  /// Whether the browser runs on a kept profile.
  pub fn kept(&self) -> bool {
    !self.throwaway
  }

  pub fn exited(&mut self) -> bool {
    !matches!(self.child.try_wait(), Ok(None))
  }

  /// Wait up to a few seconds for the browser to end after `Browser.close`.
  pub async fn wait(&mut self) {
    for _ in 0..25 {
      if self.exited() {
        return;
      }
      tokio::time::sleep(Duration::from_millis(200)).await;
    }
  }

  /// Ask the browser to quit as on logging out of the system (SIGTERM),
  /// which writes its profile out, and wait up to five seconds.
  fn terminate(&mut self) {
    #[cfg(unix)]
    {
      // SAFETY: a plain signal to our own child process.
      unsafe {
        libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM);
      }
      for _ in 0..50 {
        if self.exited() {
          return;
        }
        std::thread::sleep(Duration::from_millis(100));
      }
    }
  }
}

impl Drop for Browser {
  fn drop(&mut self) {
    if !self.exited() {
      if !self.throwaway {
        self.terminate();
      }
      if !self.exited() {
        let _ = self.child.kill();
      }
      let _ = self.child.wait();
    }
    if self.throwaway {
      let _ = std::fs::remove_dir_all(&self.profile);
    }
  }
}

/// A new empty profile in the temporary directory.
fn throwaway() -> Result<PathBuf> {
  static COUNT: AtomicU32 = AtomicU32::new(0);
  let n = COUNT.fetch_add(1, Ordering::Relaxed);
  let dir = std::env::temp_dir().join(format!("atp-browser-{}-{n}", std::process::id()));
  let _ = std::fs::remove_dir_all(&dir);
  private_dir(&dir)?;
  Ok(dir)
}

/// Lock the kept profile `dir` for this process; `None` when another
/// process holds it.
fn lock(dir: &Path) -> Result<Option<File>> {
  private_dir(dir)?;
  let file = File::options()
    .create(true)
    .truncate(false)
    .write(true)
    .open(dir.join(LOCK_FILE))?;
  match file.try_lock() {
    Ok(()) => Ok(Some(file)),
    Err(std::fs::TryLockError::WouldBlock) => Ok(None),
    Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
  }
}

/// Whether another process uses the kept profile `dir` now.
pub fn in_use(dir: &Path) -> bool {
  dir.is_dir() && matches!(lock(dir), Ok(None))
}

/// Create `dir` (and its parents) readable by this user only.
fn private_dir(dir: &Path) -> std::io::Result<()> {
  let mut builder = std::fs::DirBuilder::new();
  builder.recursive(true);
  #[cfg(unix)]
  std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
  builder.create(dir)
}

/// `proxy` as Chrome's `--proxy-server` takes it: no credentials, `socks5`
/// for `socks5h`.
pub fn proxy_server(proxy: &str) -> Option<String> {
  let mut parsed = url::Url::parse(proxy).ok()?;
  if !parsed.username().is_empty() {
    output::note("the browser cannot use the proxy's password; it may ask for it");
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
  }
  let scheme = match parsed.scheme() {
    "socks5h" => "socks5",
    other => other,
  };
  Some(format!(
    "{scheme}://{}:{}",
    parsed.host_str()?,
    parsed.port_or_known_default()?
  ))
}

/// Open `url` in the default browser; `false` when no opener worked.
pub fn open_default(url: &str) -> bool {
  let mut cmd = if cfg!(target_os = "macos") {
    Command::new("open")
  } else if cfg!(windows) {
    let mut c = Command::new("rundll32");
    c.arg("url.dll,FileProtocolHandler");
    c
  } else {
    Command::new("xdg-open")
  };
  cmd
    .arg(url)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .is_ok()
}
