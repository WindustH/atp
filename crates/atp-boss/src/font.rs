//! The digits BOSS Zhipin writes with its `kanzhun-mix` font. Where a
//! page hides the figures of the pay (`-K` on screen), it writes them
//! as private characters U+E031 to U+E03A, which that font (a static
//! file of the site, img.bosszhipin.com/static/file/2023/…) draws as
//! the digits 0 to 9 in that order: the map was read off the font's
//! glyphs once and does not change with the font's address.

/// The first of the ten private characters: `0`.
const ZERO: u32 = 0xE031;

/// The digit a character of the font stands for.
fn digit(c: char) -> Option<char> {
  let offset = (c as u32).checked_sub(ZERO)?;
  char::from_digit(offset, 10)
}

/// `text` with the font's private characters as the digits they show.
pub fn decode(text: &str) -> String {
  text.chars().map(|c| digit(c).unwrap_or(c)).collect()
}
