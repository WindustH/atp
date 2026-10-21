//! Captions as TikTok Studio's editor sends them: the plain `text`, its
//! hashtags and @-mentions as `text_extra` entries (offsets in UTF-16
//! units, as JavaScript counts them; `type` 1 for a hashtag, 0 for a
//! mention) and the `markup_text` the editor keeps (`<h id="N">#name</h>`,
//! `<m id="N">@name</m>`, the text between HTML-escaped, `<br>` for line
//! breaks). A word after `#` is a hashtag, as the page reads typed ones
//! (`#` then anything but spaces, `#`, `@` and `<`). A typed `@name` stays
//! plain text: an editor mention carries the account's id, which only the
//! mentions of a caption TikTok sent back have.

use atp_core::{Error, Result, Value, ValueExt, json};

/// The longest caption the editor takes (JavaScript string length).
pub const TEXT_MAX: usize = 4000;
const HASHTAG: i64 = 1;
const MENTION: i64 = 0;

/// An @-mention of a caption TikTok sent back: `@name` and the account's id.
#[derive(Debug, Clone, PartialEq)]
pub struct Mention {
  pub name: String,
  pub user_id: String,
}

/// A caption ready to send.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
  pub text: String,
  pub text_extra: Vec<Value>,
  pub markup: String,
}

/// One hashtag or mention in the text: its byte range and what it is.
struct Entity {
  start: usize,
  end: usize,
  user_id: Option<String>,
}

impl Caption {
  /// `body` with `topics` appended as hashtags; `mentions` the @-mentions
  /// that stay linked where their `@name` still is in the text.
  pub fn new(body: &str, topics: &[String], mentions: &[Mention]) -> Self {
    let mut text = body.trim().to_owned();
    if !topics.is_empty() {
      let tags: Vec<String> = topics.iter().map(|t| format!("#{t}")).collect();
      if !text.is_empty() {
        text.push(' ');
      }
      text.push_str(&tags.join(" "));
    }
    let mut entities = hashtags(&text);
    entities.extend(linked(&text, mentions, &entities));
    entities.sort_by_key(|e| e.start);
    let mut text_extra = Vec::new();
    let mut markup = String::new();
    let mut at = 0;
    for (i, e) in entities.iter().enumerate() {
      let word = &text[e.start..e.end];
      markup.push_str(&escape(&text[at..e.start]));
      let (tag, kind, user, hashtag) = match &e.user_id {
        Some(id) => ("m", MENTION, id.clone(), String::new()),
        None => ("h", HASHTAG, String::new(), word[1..].to_owned()),
      };
      markup.push_str(&format!("<{tag} id=\"{i}\">{}</{tag}>", escape(word)));
      text_extra.push(json!({
        "tag_id": i.to_string(),
        "start": utf16(&text[..e.start]),
        "end": utf16(&text[..e.end]),
        "user_id": user,
        "type": kind,
        "hashtag_name": hashtag,
      }));
      at = e.end;
    }
    markup.push_str(&escape(&text[at..]));
    Self {
      markup: markup.replace('\n', "<br>"),
      text,
      text_extra,
    }
  }

  /// The hashtags, without `#`.
  pub fn hashtags(&self) -> Vec<String> {
    self
      .text_extra
      .iter()
      .filter(|e| e.i64("type") == Some(HASHTAG))
      .filter_map(|e| e.str("hashtag_name"))
      .collect()
  }

  /// Fails when the editor would not take it.
  pub fn check(&self) -> Result<()> {
    if utf16(&self.text) > TEXT_MAX {
      return Err(Error::input(format!(
        "a TikTok caption takes at most {TEXT_MAX} characters"
      )));
    }
    Ok(())
  }
}

/// The names of `--topic`, without `#`; a hashtag is one word.
pub fn topic_names(topics: &[String]) -> Result<Vec<String>> {
  let mut names = Vec::new();
  for topic in topics {
    let name = topic.trim().trim_start_matches('#').trim();
    if name.is_empty() {
      continue;
    }
    if name.chars().any(|c| ends_tag(c) || c == '#') {
      return Err(Error::input(format!(
        "not a TikTok hashtag: {topic} (one word, without spaces, # or @)"
      )));
    }
    names.push(name.to_owned());
  }
  Ok(names)
}

/// `text` without its hashtags and the spaces they leave.
pub fn without_hashtags(text: &str) -> String {
  let mut out = String::new();
  let mut at = 0;
  for e in hashtags(text) {
    push_part(&mut out, &text[at..e.start]);
    at = e.end;
  }
  push_part(&mut out, &text[at..]);
  out.trim().to_owned()
}

/// Add a stretch of text between hashtags: no spaces at a line's ends,
/// one space between words.
fn push_part(out: &mut String, part: &str) {
  if part.starts_with(['\n', '\r']) {
    out.truncate(out.trim_end_matches([' ', '\t']).len());
  }
  let part = if out.is_empty() || out.ends_with('\n') {
    part.trim_start_matches([' ', '\t'])
  } else {
    part
  };
  if out.ends_with([' ', '\t']) {
    out.push_str(part.trim_start_matches([' ', '\t']));
  } else {
    out.push_str(part);
  }
}

/// The @-mentions of a caption TikTok sent back (`text_extra` entries of
/// `type` 0 with a user id), named as the text has them.
pub fn mentions(text: &str, extra: &[Value]) -> Vec<Mention> {
  let units: Vec<u16> = text.encode_utf16().collect();
  extra
    .iter()
    .filter(|e| e.i64("type") == Some(MENTION))
    .filter_map(|e| {
      let user_id = e.str("user_id").filter(|id| !id.is_empty() && id != "0")?;
      let (start, end) = (e.u64("start")? as usize, e.u64("end")? as usize);
      let name = String::from_utf16(units.get(start..end)?).ok()?;
      name.starts_with('@').then_some(Mention { name, user_id })
    })
    .collect()
}

/// The hashtags of `text`: `#` and the run of characters after it.
fn hashtags(text: &str) -> Vec<Entity> {
  let mut found = Vec::new();
  let mut chars = text.char_indices().peekable();
  while let Some((start, c)) = chars.next() {
    if c != '#' {
      continue;
    }
    let mut end = start + 1;
    while let Some(&(i, next)) = chars.peek() {
      if ends_tag(next) || next == '#' {
        break;
      }
      end = i + next.len_utf8();
      chars.next();
    }
    if end > start + 1 {
      found.push(Entity {
        start,
        end,
        user_id: None,
      });
    }
  }
  found
}

/// Where the `mentions` are in `text`, in order, outside `taken`.
fn linked(text: &str, mentions: &[Mention], taken: &[Entity]) -> Vec<Entity> {
  let mut found: Vec<Entity> = Vec::new();
  for m in mentions {
    let mut from = 0;
    while let Some(i) = text[from..].find(&m.name) {
      let start = from + i;
      let end = start + m.name.len();
      from = end;
      let clash = |e: &Entity| start < e.end && e.start < end;
      // A longer name (`@ab` within `@abc`) is another account.
      let whole = text[end..].chars().next().is_none_or(ends_mention);
      if whole && !taken.iter().any(clash) && !found.iter().any(clash) {
        found.push(Entity {
          start,
          end,
          user_id: Some(m.user_id.clone()),
        });
        break;
      }
    }
  }
  found
}

/// What ends a hashtag besides `#`: spaces, `@`, `<`.
fn ends_tag(c: char) -> bool {
  c.is_whitespace() || c == '@' || c == '<'
}

fn ends_mention(c: char) -> bool {
  !(c.is_alphanumeric() || c == '_' || c == '.')
}

fn escape(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
}

/// Length in UTF-16 units, as JavaScript counts it.
pub fn utf16(s: &str) -> usize {
  s.encode_utf16().count()
}
