//! Video chapters (视频章节) as the creator platform takes them: on a new
//! video through the publish form's chapter field, on a published one
//! through the manage page's chapter editor (作品管理 → 编辑章节), which
//! sends the whole list to `chapter/upsert` (reviewed again) or removes
//! it with `chapter/delete/`.
//!
//! The rules are the chapter editor's: videos of a minute or more, at most
//! 20 chapters, at least 5 s apart and inside the video, titles of 12 at
//! most counted as the editor counts them (Latin letters, digits, `+` and
//! `-` half). The first chapter may start after 0:00; the player then
//! shows the part before it untitled.

use atp_core::chapters::{self, Chapter, Rules};
use atp_core::{Error, Result, Value, ValueExt};

/// The chapter editor's limits (`MAX_CHAPTER_LENGTH`, the 5 s spacing, the
/// one-minute floor); titles are checked by [`title_len`].
const RULES: Rules = Rules {
  platform: "Douyin",
  min_count: 1,
  max_count: 20,
  first_at_zero: false,
  min_length: 5,
  title_max: 0,
  min_video: 60,
};
/// The longest chapter title, in the editor's units.
const TITLE_MAX: usize = 12;

/// Fails on the first rule the chapters break; `duration` in seconds when
/// known.
pub(super) fn check(list: &[Chapter], duration: Option<u32>) -> Result<()> {
  chapters::check(list, &RULES, duration)?;
  if let Some(c) = list.iter().find(|c| title_len(&c.title) > TITLE_MAX) {
    return Err(Error::input(format!(
      "chapter titles on Douyin take at most {TITLE_MAX} characters (Latin letters and digits \
       count half): `{}` counts {}",
      c.title,
      title_len(&c.title)
    )));
  }
  Ok(())
}

/// A title's length as the chapter editor counts it: Latin letters,
/// digits, `+` and `-` count half (rounded up overall), any other
/// character one per UTF-16 unit (JavaScript string length).
pub(super) fn title_len(title: &str) -> usize {
  let (half, whole) = title.chars().fold((0usize, 0), |(half, whole), c| {
    if c.is_ascii_alphanumeric() || c == '+' || c == '-' {
      (half + 1, whole)
    } else {
      (half, whole + c.len_utf16())
    }
  });
  whole + half.div_ceil(2)
}

/// A work's chapters as the creator platform gives them (`chapter_list`,
/// starts in milliseconds).
pub(super) fn of_work(aweme: &Value) -> Vec<Chapter> {
  aweme
    .list("chapter_list")
    .iter()
    .filter_map(|c| {
      Some(Chapter {
        start: u32::try_from(c.u64("timestamp")? / 1000).ok()?,
        title: c.str("desc").unwrap_or_default(),
      })
    })
    .collect()
}
