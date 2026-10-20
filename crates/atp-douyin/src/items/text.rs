//! The text fields of an item (`text`, `text_extra`, `challenges` ...) as
//! the page derives them from its editor (`eA` and the editor's read-back
//! in the creator bundle's text module): hashtags as `#name` spans,
//! @-mentions as `@name` spans, offsets in UTF-16 units (JavaScript string
//! lengths), an image post's title joined with `。`, an article's topics
//! counted in code points. The edit page fills its editor back from a
//! published work the same way; the links it does not know it passes
//! over, their words left as plain text.

use atp_core::{Error, ErrorCode, Result, Value, ValueExt, json};

use super::{Tag, Words};

/// `text_extra` types the page's editor knows (its `Friend`, `Hashtag`,
/// `ImageTitle`, `ImageTitleDiviver`, `Activity`): an @-mention, a
/// hashtag, an image post's title and the `。` after it, and an
/// activity's hashtag, which it sends back as a hashtag.
const MENTION: i64 = 0;
const HASHTAG: i64 = 1;
const IMAGE_TITLE: i64 = 7;
const IMAGE_TITLE_DIVIDER: i64 = 8;
pub const ACTIVITY: i64 = 9;
/// Links Douyin puts in a text itself, which the editor does not know
/// (douyin.com's text types `Search` and `IMGroup`): a search word (搜索词,
/// a word that opens a search for it) and a group chat.
const SEARCH: i64 = 5;
const GROUP_CHAT: i64 = 16;

/// The text fields the publish form derives from its editor.
#[derive(Debug)]
pub struct Text {
  pub text: String,
  pub caption: String,
  pub title: String,
  pub extra: Value,
  pub challenges: Value,
  /// The mentioned users' ids (`mentions`).
  pub mentions: Value,
  pub source: String,
}

/// An @-mention as the editor holds it: the name shown after `@` and the
/// user's id (with `at_source` when the work has one), which the page
/// reads back from the span it loaded.
#[derive(Debug, Clone)]
pub struct Mention {
  pub name: String,
  pub user_id: Value,
  pub at_source: Option<Value>,
}

/// The editor's content: its text, the hashtags and the @-mentions in it
/// (`start`, `end` in UTF-16 units), and the links of the loaded text it
/// passed over.
#[derive(Debug, Clone, Default)]
pub struct Editor {
  pub text: String,
  pub spans: Vec<(usize, usize, Tag)>,
  pub mentions: Vec<(usize, usize, Mention)>,
  pub links: Vec<Link>,
}

/// A `text_extra` entry of a type the editor does not know (a search
/// word, type 5): the page's loader leaves its words in the text as plain
/// text and keeps nothing else of it, so it is not sent back.
#[derive(Debug, Clone)]
pub struct Link {
  pub kind: i64,
  /// The words it marks in the description (else its `search_text`).
  pub words: String,
}

impl Link {
  fn of(e: &Value, kind: i64, desc: &str) -> Self {
    let units: Vec<u16> = desc.encode_utf16().collect();
    let end = offset(e, "end").min(units.len());
    let start = offset(e, "start").min(end);
    let words = String::from_utf16_lossy(&units[start..end])
      .trim()
      .to_owned();
    let words = if words.is_empty() {
      e.str("search_text").unwrap_or_default()
    } else {
      words
    };
    Self { kind, words }
  }

  /// What it is, as the summary names it: `search word 内核态 (text_extra
  /// type 5)`.
  pub fn name(&self) -> String {
    let what = match self.kind {
      SEARCH => "search word",
      GROUP_CHAT => "group chat link",
      _ => "link",
    };
    match self.words.as_str() {
      "" => format!("{what} (text_extra type {})", self.kind),
      words => format!("{what} {words} (text_extra type {})", self.kind),
    }
  }
}

impl Editor {
  /// The body, then each tag as `#name`, as typed in the editor.
  pub fn typed(body: &str, tags: &[Tag]) -> Self {
    let mut text = body.trim().to_owned();
    let mut spans = Vec::new();
    for tag in tags {
      if !text.is_empty() {
        text.push(' ');
      }
      let start = utf16(&text);
      text.push('#');
      text.push_str(&tag.name);
      spans.push((start, utf16(&text), tag.clone()));
    }
    Self {
      text,
      spans,
      ..Self::default()
    }
  }

  /// Keep the @-mentions of `old` whose `@name` is still in the text, in
  /// order and outside the hashtags, at their new place; the names of the
  /// others, which go as plain text (as when one is deleted in the page's
  /// editor).
  pub fn keep_mentions(&mut self, old: &[(usize, usize, Mention)]) -> Vec<String> {
    let units: Vec<u16> = self.text.encode_utf16().collect();
    let (mut from, mut dropped) = (0, Vec::new());
    for (_, _, mention) in old {
      let needle: Vec<u16> = format!("@{}", mention.name).encode_utf16().collect();
      let found = (from..=units.len().saturating_sub(needle.len()))
        .filter(|at| units[*at..].starts_with(&needle))
        .map(|at| (at, at + needle.len()))
        .find(|(start, end)| !self.spans.iter().any(|(s, e, _)| start < e && s < end));
      match found {
        Some((start, end)) => {
          self.mentions.push((start, end, mention.clone()));
          from = end;
        }
        None => dropped.push(mention.name.clone()),
      }
    }
    dropped
  }

  pub fn tags(&self) -> Vec<Tag> {
    self.spans.iter().map(|(_, _, tag)| tag.clone()).collect()
  }

  /// `@name` of each mention, in order.
  pub fn mention_names(&self) -> Vec<String> {
    self
      .mentions
      .iter()
      .map(|(_, _, m)| format!("@{}", m.name))
      .collect()
  }

  /// The activities (活动) the text joins: the ids of its hashtags that
  /// are an activity's (`challenge_ids` of `item/info`'s `activity_list`),
  /// in order, as the page's `activity` lists them.
  pub fn activity(&self, challenge_ids: &[u64]) -> Value {
    json!(
      self
        .spans
        .iter()
        .filter(|(_, _, t)| t.id != 0 && challenge_ids.contains(&t.id))
        .map(|(_, _, t)| t.id)
        .collect::<Vec<_>>()
    )
  }

  /// The text without its hashtags (and the space before each).
  pub fn body(&self) -> String {
    let units: Vec<u16> = self.text.encode_utf16().collect();
    let (mut out, mut at) = (Vec::new(), 0);
    for (start, end, _) in &self.spans {
      let mut cut = (*start).min(units.len());
      while cut > at && units[cut - 1] == u16::from(b' ') {
        cut -= 1;
      }
      out.extend_from_slice(&units[at.min(cut)..cut]);
      at = (*end).min(units.len());
    }
    out.extend_from_slice(&units[at.min(units.len())..]);
    String::from_utf16_lossy(&out).trim().to_owned()
  }
}

/// The text of a video (`eA` with a title): `title caption`, hashtag
/// offsets moved past the title, `caption_*` offsets within the caption.
pub fn video_text(words: Words) -> Text {
  video_fields(words.title, &Editor::typed(words.body, words.tags))
}

/// [`video_text`] of an editor's content.
pub fn video_fields(title: &str, editor: &Editor) -> Text {
  let title = title.trim();
  let caption = &editor.text;
  let tags = editor.tags();
  let shift = if title.is_empty() {
    0
  } else {
    utf16(title) + 1
  };
  let text = if title.is_empty() {
    caption.clone()
  } else {
    format!("{title} {caption}")
  };
  Text {
    text,
    caption: caption.clone(),
    title: title.to_owned(),
    extra: Value::Array(spans_extra(editor, shift)),
    challenges: challenges(&tags),
    mentions: mentions(editor),
    source: sources(&tags),
  }
}

/// The text of an image post (`eA`, then the title joined with `。`).
pub fn image_text(words: Words) -> Text {
  image_fields(words.title, &Editor::typed(words.body, words.tags))
}

/// [`image_text`] of an editor's content.
pub fn image_fields(title: &str, editor: &Editor) -> Text {
  let title = title.trim();
  let caption = &editor.text;
  let tags = editor.tags();
  let mut prefix = title.to_owned();
  let mut extra = Vec::new();
  if !title.is_empty() {
    let len = utf16(title);
    extra.push(span(0, len, IMAGE_TITLE));
    if !title.ends_with('。') {
      prefix.push('。');
      extra.push(span(len, len + 1, IMAGE_TITLE_DIVIDER));
    }
  }
  extra.extend(spans_extra(editor, utf16(&prefix)));
  Text {
    text: format!("{prefix}{caption}"),
    caption: caption.clone(),
    title: title.to_owned(),
    extra: Value::Array(extra),
    challenges: challenges(&tags),
    mentions: mentions(editor),
    source: sources(&tags),
  }
}

/// The `text_extra` entries of the editor's hashtags and @-mentions in
/// text order, `shift` units into the full text, `caption_*` within the
/// editor's.
fn spans_extra(editor: &Editor, shift: usize) -> Vec<Value> {
  let tags = editor.spans.iter().map(|(start, end, tag)| {
    (
      *start,
      hashtag(tag, start + shift, end + shift, *start, *end),
    )
  });
  let mentions = editor.mentions.iter().map(|(start, end, m)| {
    let mut entry = json!({
      "start": start + shift,
      "end": end + shift,
      "type": MENTION,
      "hashtag_name": "",
      "hashtag_id": 0,
      "user_id": m.user_id,
    });
    if let Some(source) = &m.at_source {
      entry["at_source"] = source.clone();
    }
    entry["caption_start"] = json!(start);
    entry["caption_end"] = json!(end);
    (*start, entry)
  });
  let mut all: Vec<(usize, Value)> = tags.chain(mentions).collect();
  all.sort_by_key(|(start, _)| *start);
  all.into_iter().map(|(_, entry)| entry).collect()
}

/// The ids of the mentioned users that have one (`mentions`).
fn mentions(editor: &Editor) -> Value {
  let ids: Vec<Value> = editor
    .mentions
    .iter()
    .map(|(_, _, m)| m.user_id.clone())
    .filter(|id| match id {
      Value::String(s) => !s.is_empty(),
      Value::Number(n) => n.as_f64() != Some(0.0),
      _ => false,
    })
    .collect();
  Value::Array(ids)
}

/// A published video's title and editor content, as the edit page fills
/// them in: the caption (the description when there is neither a title
/// nor a caption) with its spans moved into it, the links it does not
/// know passed over.
pub fn video_editor(aweme: &Value) -> Result<(String, Editor)> {
  let title = aweme.str("item_title").unwrap_or_default();
  let caption = aweme.str("caption").unwrap_or_default();
  let desc = aweme.str("desc").unwrap_or_default();
  let text = if title.is_empty() && caption.is_empty() {
    desc.clone()
  } else {
    caption.clone()
  };
  // Offsets into `desc` move by what precedes the caption in it.
  let shift = utf16(&desc).saturating_sub(utf16(&caption));
  let mut editor = Editor {
    text,
    ..Editor::default()
  };
  for e in aweme.list("text_extra") {
    let kind = e.i64("type").unwrap_or(-1);
    if kind == IMAGE_TITLE || kind == IMAGE_TITLE_DIVIDER || editor.passes_over(e, kind, &desc) {
      continue;
    }
    let (cs, ce) = (offset(e, "caption_start"), offset(e, "caption_end"));
    let (start, end) = if caption.is_empty() {
      (offset(e, "start"), offset(e, "end"))
    } else if cs != 0 || ce != 0 {
      (cs, ce)
    } else {
      let at = |key| offset(e, key).saturating_sub(shift);
      (at("start"), at("end"))
    };
    editor.load(e, kind, (start, end))?;
  }
  editor.sort();
  Ok((title, editor))
}

/// A published image post's title and editor content: its description is
/// the title, `。` and the text (`text_extra` types 7 and 8 mark them);
/// the links the editor does not know are passed over.
pub fn image_editor(aweme: &Value) -> Result<(String, Editor)> {
  let desc = aweme.str("desc").unwrap_or_default();
  let units: Vec<u16> = desc.encode_utf16().collect();
  let extra = aweme.list("text_extra");
  let mark = |kind| {
    extra
      .iter()
      .find(|e| e.i64("type") == Some(kind))
      .map(|e| (offset(e, "start"), offset(e, "end")))
  };
  let title_end = mark(IMAGE_TITLE).map_or(0, |(_, end)| end.min(units.len()));
  let body_at = mark(IMAGE_TITLE_DIVIDER).map_or(title_end, |(_, end)| end.min(units.len()));
  let title = String::from_utf16_lossy(&units[..title_end]);
  let text = String::from_utf16_lossy(&units[body_at..]);
  let mut editor = Editor {
    text,
    ..Editor::default()
  };
  for e in extra {
    let kind = e.i64("type").unwrap_or(-1);
    if kind == IMAGE_TITLE || kind == IMAGE_TITLE_DIVIDER || editor.passes_over(e, kind, &desc) {
      continue;
    }
    let start = offset(e, "start").saturating_sub(body_at);
    let end = offset(e, "end").saturating_sub(body_at);
    editor.load(e, kind, (start, end))?;
  }
  editor.sort();
  Ok((title, editor))
}

impl Editor {
  /// Whether the editor passes over a loaded `text_extra` entry: one of a
  /// type it does not know (the page's loader takes only its own), whose
  /// words stay in the text as plain text. It goes in `links`, with the
  /// words it marks in `desc`.
  fn passes_over(&mut self, e: &Value, kind: i64, desc: &str) -> bool {
    let known = [MENTION, HASHTAG, IMAGE_TITLE, IMAGE_TITLE_DIVIDER, ACTIVITY];
    if known.contains(&kind) {
      return false;
    }
    self.links.push(Link::of(e, kind, desc));
    true
  }

  /// Take a loaded `text_extra` span at `start..end` of the text: a
  /// hashtag (an activity's too, which the editor reads back as `#name`)
  /// or an @-mention, whose name is the text after its `@`, as the page
  /// shows it.
  fn load(&mut self, e: &Value, kind: i64, (start, end): (usize, usize)) -> Result<()> {
    match kind {
      HASHTAG | ACTIVITY => self.spans.push((
        start,
        end,
        Tag {
          name: e.str("hashtag_name").unwrap_or_default(),
          id: e.u64("hashtag_id").unwrap_or(0),
        },
      )),
      MENTION => {
        let units: Vec<u16> = self.text.encode_utf16().collect();
        let at = units
          .get(start..end)
          .filter(|s| s.first() == Some(&u16::from(b'@')));
        let Some(span) = at else {
          return Err(Error::new(
            ErrorCode::UnsupportedOperation,
            "an @-mention in this work's text is not where Douyin says; change the work on creator.douyin.com",
          ));
        };
        let mention = Mention {
          name: String::from_utf16_lossy(&span[1..]),
          user_id: e.at("user_id").clone(),
          at_source: Some(e.at("at_source").clone()).filter(|s| !s.is_null()),
        };
        self.mentions.push((start, end, mention));
      }
      _ => {}
    }
    Ok(())
  }

  fn sort(&mut self) {
    self.spans.sort_by_key(|(start, _, _)| *start);
    self.mentions.sort_by_key(|(start, _, _)| *start);
  }
}

fn offset(e: &Value, key: &str) -> usize {
  e.u64(key).unwrap_or(0) as usize
}

/// An article's `text_extra`: the title span, then each topic after it
/// (offsets in code points, `caption_*` without the title).
pub fn article_extra(title: &str, tags: &[Tag]) -> Value {
  if tags.is_empty() {
    return json!([]);
  }
  let len = title.chars().count();
  let mut extra = vec![span(0, len, IMAGE_TITLE)];
  let (mut at, mut caption_at) = (len + 1, 0);
  for tag in tags {
    let n = tag.name.chars().count();
    extra.push(json!({
      "hashtag_id": tag.id,
      "hashtag_name": tag.name,
      "type": HASHTAG,
      "start": at,
      "end": at + n + 1,
      "caption_start": caption_at,
      "caption_end": caption_at + n + 1,
    }));
    at += n + 2;
    caption_at += n + 2;
  }
  Value::Array(extra)
}

fn hashtag(tag: &Tag, start: usize, end: usize, caption_start: usize, caption_end: usize) -> Value {
  json!({
    "start": start,
    "end": end,
    "type": HASHTAG,
    "hashtag_name": tag.name,
    "hashtag_id": tag.id,
    "user_id": "",
    "caption_start": caption_start,
    "caption_end": caption_end,
  })
}

fn span(start: usize, end: usize, kind: i64) -> Value {
  json!({ "start": start, "end": end, "hashtag_id": 0, "hashtag_name": "", "type": kind })
}

/// Ids of the hashtags Douyin already knows.
fn challenges(tags: &[Tag]) -> Value {
  json!(
    tags
      .iter()
      .filter(|t| t.id != 0)
      .map(|t| t.id)
      .collect::<Vec<_>>()
  )
}

/// How each hashtag was added: typed (`search`).
fn sources(tags: &[Tag]) -> String {
  vec!["search"; tags.len()].join("/")
}

fn utf16(s: &str) -> usize {
  s.encode_utf16().count()
}
