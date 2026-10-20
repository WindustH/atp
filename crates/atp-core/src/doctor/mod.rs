//! `atp doctor`: what ATP depends on, checked in one go: the version,
//! where it keeps its files and who may read the sessions, each saved
//! session (the `status` check, one platform after another), ffmpeg, a
//! Chromium-family browser, the proxy, and the network and the clock (one
//! request). `--offline` skips everything that goes online. The exit code
//! is 1 when something needs attention.

mod local;
mod online;

use std::process::ExitCode;

use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets};
use serde::Serialize;

use crate::app::App;
use crate::cli::GlobalArgs;
use crate::model::Data;
use crate::output::{self, Format};

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
  Ok,
  /// Works, but something is missing or risky.
  Warn,
  /// Does not work.
  Fail,
  /// Not checked (`--offline`, nothing to check here).
  Skip,
}

impl Status {
  fn attention(self) -> bool {
    matches!(self, Status::Warn | Status::Fail)
  }

  fn cell(self) -> Cell {
    let (text, color) = match self {
      Status::Ok => ("ok", Color::Green),
      Status::Warn => ("warn", Color::Yellow),
      Status::Fail => ("fail", Color::Red),
      Status::Skip => ("skip", Color::DarkGrey),
    };
    Cell::new(text).fg(color)
  }
}

/// One line of the report.
#[derive(Debug, Serialize)]
pub(crate) struct Check {
  pub check: &'static str,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub platform: Option<&'static str>,
  pub status: Status,
  pub detail: String,
}

impl Check {
  pub fn new(check: &'static str, status: Status, detail: impl Into<String>) -> Self {
    Self {
      check,
      platform: None,
      status,
      detail: detail.into(),
    }
  }

  pub fn on(mut self, platform: &'static str) -> Self {
    self.platform = Some(platform);
    self
  }
}

#[derive(Serialize)]
struct Report {
  version: &'static str,
  /// Nothing needs attention.
  healthy: bool,
  /// Sessions and data files, one folder per platform.
  config_dir: String,
  cache_dir: String,
  checks: Vec<Check>,
}

/// `atp doctor [--offline]`.
pub(crate) async fn run(app: &App, global: &GlobalArgs, offline: bool) -> ExitCode {
  let (config, cache) = crate::store::roots();
  let mut checks = vec![Check::new(
    "version",
    Status::Ok,
    format!("atp {}", app.version),
  )];
  checks.extend(local::dirs(&config, &cache));
  checks.extend(local::session_files(app));
  checks.extend(online::sessions(app, global, offline).await);
  checks.push(local::ffmpeg());
  checks.push(local::browser(app));
  checks.push(online::proxy(global));
  checks.extend(online::network(global, offline).await);
  let healthy = !checks.iter().any(|c| c.status.attention());
  let report = Report {
    version: app.version,
    healthy,
    config_dir: config.display().to_string(),
    cache_dir: cache.display().to_string(),
    checks,
  };
  print(&report, global.output_format());
  if healthy {
    ExitCode::SUCCESS
  } else {
    ExitCode::FAILURE
  }
}

fn print(report: &Report, format: Format) {
  match format {
    Format::Table => table(report),
    // Rows are the checks.
    Format::Jsonl | Format::Csv => output::emit(
      format,
      None,
      &Data::Value(serde_json::to_value(&report.checks).unwrap_or_default()),
    ),
    _ => output::emit(
      format,
      None,
      &Data::Value(serde_json::to_value(report).unwrap_or_default()),
    ),
  }
}

/// The checks as a table, then what needs attention.
fn table(report: &Report) {
  let mut t = Table::new();
  t.load_style(presets::NOTHING)
    .set_content_arrangement(ContentArrangement::Dynamic);
  t.set_header(["CHECK", "STATUS", "DETAIL"].map(|h| Cell::new(h).add_attribute(Attribute::Bold)));
  for c in &report.checks {
    let name = match c.platform {
      Some(p) => format!("{} {p}", c.check),
      None => c.check.to_owned(),
    };
    t.add_row(vec![Cell::new(name), c.status.cell(), Cell::new(&c.detail)]);
  }
  println!("{t}");
  let attention = report
    .checks
    .iter()
    .filter(|c| c.status.attention())
    .count();
  println!();
  match attention {
    0 => println!("Everything looks fine."),
    1 => println!("1 thing needs attention."),
    n => println!("{n} things need attention."),
  }
}
