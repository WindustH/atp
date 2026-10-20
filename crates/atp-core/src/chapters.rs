//! Video chapters (章节): the `0:00 Title` lines `post` and `edit` read
//! from `--chapters FILE`, the rules each platform holds them to, and the
//! `chapters` key of [`Post.extra`](crate::model::Post::extra) that `read`
//! fills: `[{title, start, end}]`, times in seconds.

use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::text::fmt_duration;

/// One chapter: where it starts, in whole seconds, and its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
  pub start: u32,
  pub title: String,
}

/// Chapters from text: one chapter a line, its time (`M:SS`, `MM:SS` or
/// `H:MM:SS`) then its title, optionally after ` - `, ` | ` or ` : `
/// (`0:00 Intro`, `00:00 - Intro`, `1:02:03 | Part three`). Blank lines
/// are skipped. Times must rise from line to line.
pub fn parse(text: &str) -> Result<Vec<Chapter>> {
  let mut chapters: Vec<Chapter> = Vec::new();
  for (i, line) in text.lines().enumerate() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let chapter =
      parse_line(line).map_err(|e| Error::input(format!("chapters, line {}: {e}", i + 1)))?;
    if let Some(last) = chapters.last()
      && chapter.start <= last.start
    {
      return Err(Error::input(format!(
        "chapters, line {}: {} does not come after {} on the line before; give the chapters in order, each at its own time",
        i + 1,
        stamp(chapter.start),
        stamp(last.start)
      )));
    }
    chapters.push(chapter);
  }
  if chapters.is_empty() {
    return Err(Error::input(
      "no chapters given: write one `0:00 Title` line per chapter",
    ));
  }
  Ok(chapters)
}

fn parse_line(line: &str) -> std::result::Result<Chapter, String> {
  let (time, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
  let start = parse_time(time)
    .ok_or_else(|| format!("`{line}` does not start with a time (M:SS or H:MM:SS)"))?;
  let rest = rest.trim_start();
  let title = ["-", "–", "—", "|", ":"]
    .iter()
    .find_map(|sep| {
      rest
        .strip_prefix(sep)
        .filter(|t| t.starts_with(char::is_whitespace) || t.is_empty())
    })
    .unwrap_or(rest)
    .trim();
  if title.is_empty() {
    return Err(format!("the chapter at {} has no title", stamp(start)));
  }
  Ok(Chapter {
    start,
    title: title.to_owned(),
  })
}

/// Seconds of `M:SS`, `MM:SS` or `H:MM:SS`.
pub fn parse_time(s: &str) -> Option<u32> {
  let parts: Vec<&str> = s.split(':').collect();
  if !(2..=3).contains(&parts.len())
    || parts
      .iter()
      .any(|p| p.is_empty() || p.len() > 4 || !p.bytes().all(|b| b.is_ascii_digit()))
  {
    return None;
  }
  let numbers: Vec<u32> = parts
    .iter()
    .map(|p| p.parse().ok())
    .collect::<Option<_>>()?;
  let (h, m, s) = match numbers[..] {
    [m, s] => (0, m, s),
    [h, m, s] if m < 60 && parts[1].len() == 2 => (h, m, s),
    _ => return None,
  };
  (s < 60 && parts.last()?.len() == 2).then_some(h * 3600 + m * 60 + s)
}

/// `0:00`, `1:02`, `1:02:03`.
pub fn stamp(seconds: u32) -> String {
  fmt_duration(f64::from(seconds))
}

/// The chapters as text lines (`0:00 Intro`), as `--chapters` reads them
/// and YouTube descriptions hold them.
pub fn lines(chapters: &[Chapter]) -> String {
  chapters
    .iter()
    .map(|c| format!("{} {}", stamp(c.start), c.title))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The chapters on one line, for the change an `edit` shows.
pub fn summary(chapters: &[Chapter]) -> String {
  chapters
    .iter()
    .map(|c| format!("{} {}", stamp(c.start), c.title))
    .collect::<Vec<_>>()
    .join(" · ")
}

/// Where a chapter ends: at the next one, the last one at `duration`.
pub fn ends(chapters: &[Chapter], duration: u32) -> Vec<u32> {
  chapters
    .iter()
    .skip(1)
    .map(|c| c.start)
    .chain(std::iter::once(duration))
    .collect()
}

/// The `chapters` value of `Post.extra`: `[{title, start, end}]` from
/// `(title, start)` pairs in order, each `end` the next start, the last
/// one's `duration` (left out when unknown). Times in seconds.
pub fn value(chapters: &[(String, f64)], duration: Option<f64>) -> Value {
  let list: Vec<Value> = chapters
    .iter()
    .enumerate()
    .map(|(i, (title, start))| {
      let end = chapters.get(i + 1).map(|(_, s)| *s).or(duration);
      let mut item = json!({ "title": title, "start": start });
      if let Some(end) = end.filter(|e| e > start) {
        item["end"] = json!(end);
      }
      item
    })
    .collect();
  Value::from(list)
}

/// What a platform holds chapters to.
#[derive(Debug, Clone, Copy)]
pub struct Rules {
  /// The platform's name, for the messages.
  pub platform: &'static str,
  pub min_count: usize,
  pub max_count: usize,
  /// The first chapter must start at 0:00.
  pub first_at_zero: bool,
  /// The shortest a chapter may last, in seconds.
  pub min_length: u32,
  /// The longest title, in characters (0: no limit).
  pub title_max: usize,
  /// The shortest video that takes chapters, in seconds.
  pub min_video: u32,
}

/// Fails on the first rule `chapters` break; `duration` (seconds) is the
/// video's, when known.
pub fn check(chapters: &[Chapter], rules: &Rules, duration: Option<u32>) -> Result<()> {
  let p = rules.platform;
  let fail = |message: String| Err(Error::input(message));
  if let Some(d) = duration.filter(|d| *d < rules.min_video) {
    return fail(format!(
      "{p} takes chapters on videos of at least {} (this one lasts {})",
      stamp(rules.min_video),
      stamp(d)
    ));
  }
  if chapters.len() < rules.min_count {
    return fail(format!(
      "{p} needs at least {} chapters ({} given)",
      rules.min_count,
      chapters.len()
    ));
  }
  if chapters.len() > rules.max_count {
    return fail(format!(
      "{p} takes at most {} chapters ({} given)",
      rules.max_count,
      chapters.len()
    ));
  }
  if rules.first_at_zero && chapters.first().is_some_and(|c| c.start != 0) {
    return fail(format!("the first chapter on {p} starts at 0:00"));
  }
  if let Some(c) = chapters
    .iter()
    .find(|c| rules.title_max > 0 && c.title.chars().count() > rules.title_max)
  {
    return fail(format!(
      "chapter titles on {p} take at most {} characters: `{}`",
      rules.title_max, c.title
    ));
  }
  let ends = ends(chapters, duration.unwrap_or(u32::MAX));
  for (c, end) in chapters.iter().zip(ends) {
    if let Some(d) = duration.filter(|d| c.start >= *d) {
      return fail(format!(
        "the chapter `{}` at {} starts after the video ends ({})",
        c.title,
        stamp(c.start),
        stamp(d)
      ));
    }
    if end - c.start < rules.min_length {
      return fail(format!(
        "chapters on {p} last at least {} s: `{}` at {} lasts {} s",
        rules.min_length,
        c.title,
        stamp(c.start),
        end - c.start
      ));
    }
  }
  Ok(())
}
