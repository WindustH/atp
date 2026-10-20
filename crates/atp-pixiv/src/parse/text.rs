//! The markup of pixiv novels as plain text: `[newpage]` becomes a rule
//! between pages, `[chapter:…]` a title on its own line, ruby
//! (`[[rb:漢字 > かんじ]]`) the reading in brackets, links
//! (`[[jumpuri:text > url]]`) the text with the address, jumps
//! (`[jump:3]`) and pictures (`[pixivimage:…]`, `[uploadedimage:…]`)
//! short notes in brackets.

use std::sync::LazyLock;

use regex::{Captures, Regex};

static MARKUP: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(concat!(
    r"\[newpage\]",
    r"|\[chapter:\s*([^\]]*?)\s*\]",
    r"|\[\[rb:\s*(.+?)\s*>\s*(.+?)\s*\]\]",
    r"|\[\[jumpuri:\s*(.+?)\s*>\s*(\S+?)\s*\]\]",
    r"|\[jump:\s*(\d+)\s*\]",
    r"|\[pixivimage:\s*([\d-]+)\s*\]",
    r"|\[uploadedimage:\s*(\d+)\s*\]",
  ))
  .unwrap()
});

/// A novel's text without its markup.
pub fn novel(content: &str) -> String {
  let text = MARKUP.replace_all(content, |c: &Captures| {
    let group = |i: usize| c.get(i).map(|m| m.as_str());
    if let Some(title) = group(1) {
      return format!("\n{title}\n");
    }
    if let (Some(base), Some(reading)) = (group(2), group(3)) {
      return format!("{base}（{reading}）");
    }
    if let (Some(label), Some(url)) = (group(4), group(5)) {
      return format!("{label} ({url})");
    }
    if let Some(page) = group(6) {
      return format!("[→ page {page}]");
    }
    if let Some(work) = group(7) {
      return format!("[artwork {work}]");
    }
    if let Some(image) = group(8) {
      return format!("[image {image}]");
    }
    "\n\n---\n\n".to_owned()
  });
  text.trim().to_owned()
}
