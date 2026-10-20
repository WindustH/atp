//! Chapters in a video's description, as YouTube reads them: a list of
//! `0:00 Title` lines starting at 0:00, at least three, each at least
//! ten seconds long, in order. `edit --chapters` replaces that list in the
//! description (or appends one) and keeps the rest as it was.

use atp_core::chapters::{self, Chapter, Rules};
use atp_core::{Error, Result};

const RULES: Rules = Rules {
  platform: "YouTube",
  min_count: 3,
  max_count: usize::MAX,
  first_at_zero: true,
  min_length: 10,
  title_max: 0,
  min_video: 0,
};

/// The description with `new` chapters in place of its chapter list
/// (appended when it has none); empty `new` removes the list. `duration`
/// is the video's, in seconds, when known.
pub fn apply(description: &str, new: &[Chapter], duration: Option<u32>) -> Result<String> {
  if !new.is_empty() {
    chapters::check(new, &RULES, duration)?;
    if new.iter().any(|c| c.title.contains(['<', '>'])) {
      return Err(Error::input(
        "YouTube descriptions cannot hold `<` or `>`: take them out of the chapter titles",
      ));
    }
  }
  let lines: Vec<&str> = description.lines().collect();
  let block = block(&lines);
  let text = match (block, new.is_empty()) {
    (None, true) => {
      return Err(Error::input(
        "the description has no chapters to remove (a list of `0:00 Title` lines)",
      ));
    }
    (None, false) => {
      let head = description.trim_end();
      let sep = if head.is_empty() { "" } else { "\n\n" };
      format!("{head}{sep}{}", chapters::lines(new))
    }
    (Some((from, to)), false) => {
      let list = chapters::lines(new);
      let joined: Vec<&str> = lines[..from]
        .iter()
        .copied()
        .chain(list.lines())
        .chain(lines[to..].iter().copied())
        .collect();
      joined.join("\n")
    }
    (Some((from, to)), true) => {
      let before = lines[..from].join("\n");
      let after = lines[to..].join("\n");
      let (before, after) = (
        before.trim_end(),
        after.trim_start_matches(['\n', ' ', '\t']),
      );
      let sep = if before.is_empty() || after.is_empty() {
        ""
      } else {
        "\n\n"
      };
      format!("{before}{sep}{after}").trim_end().to_owned()
    }
  };
  Ok(text)
}

/// The chapter list of a description: the first run of two or more lines
/// starting with a time, the first of them at 0:00, as `[from, to)`.
fn block(lines: &[&str]) -> Option<(usize, usize)> {
  let timed = |line: &str| {
    let token = line.split_whitespace().next()?;
    chapters::parse_time(token)
  };
  let mut i = 0;
  while i < lines.len() {
    if timed(lines[i]) != Some(0) {
      i += 1;
      continue;
    }
    let end = (i + 1..lines.len())
      .find(|j| timed(lines[*j]).is_none())
      .unwrap_or(lines.len());
    if end - i >= 2 {
      return Some((i, end));
    }
    i = end;
  }
  None
}

/// Seconds of an ISO 8601 duration (`PT1H2M3S`, `P1DT2H`), as the Data
/// API gives `contentDetails.duration`.
pub fn iso_seconds(s: &str) -> Option<u32> {
  let rest = s.strip_prefix('P')?;
  let mut total: u64 = 0;
  let mut number = String::new();
  let mut time = false;
  for c in rest.chars() {
    match c {
      'T' => time = true,
      '0'..='9' => number.push(c),
      unit => {
        let n: u64 = number.parse().ok()?;
        number.clear();
        total += n
          * match (unit, time) {
            ('D', false) => 86_400,
            ('W', false) => 7 * 86_400,
            ('H', true) => 3600,
            ('M', true) => 60,
            ('S', true) => 1,
            _ => return None,
          };
      }
    }
  }
  u32::try_from(total).ok()
}
