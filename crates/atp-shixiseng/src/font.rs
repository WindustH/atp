//! The private font of the search's internship cards. Their titles, pay,
//! days a week, months and company sizes are written with characters of
//! Unicode's private use area (`&#xe942`, without the `;`), which the page
//! draws with a font of its own (`/interns/iconfonts/file`, WOFF). The
//! font's glyph names carry the real characters (`uni6a` is `j`, `uni4E00`
//! is 一): its `cmap` table maps each private character to a glyph and its
//! `post` table names the glyph. Nothing is drawn or recognized; the map
//! is read from the font and kept a few hours in the private cache.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Read;
use std::ops::RangeInclusive;
use std::time::Duration;

use atp_core::{Error, Result, Value};

use crate::api::{Api, WWW};

const FONT: &str = "/interns/iconfonts/file";
/// The cache entry of the map.
const CACHE: &str = "font-map";
const TTL: Duration = Duration::from_secs(6 * 3600);
/// Unicode's private use area.
const PRIVATE: RangeInclusive<u32> = 0xE000..=0xF8FF;
/// How many standard Macintosh glyph names come before a `post` table's
/// own.
const STANDARD_NAMES: usize = 258;

/// Private characters and the characters they stand for.
#[derive(Debug, Default, Clone)]
pub struct Glyphs(BTreeMap<u32, char>);

impl Glyphs {
  /// The map, from the cache unless `fresh` or it is older than a few
  /// hours, else from the site's font.
  pub async fn load(api: &Api, fresh: bool) -> Result<Self> {
    let cached = (!fresh)
      .then(|| {
        api
          .ctx
          .store
          .cache_get::<BTreeMap<String, String>>(CACHE, TTL)
      })
      .flatten()
      .and_then(|map| Self::from_cache(&map));
    if let Some(glyphs) = cached {
      return Ok(glyphs);
    }
    let rand = format!("{:.16}", rand::random::<f64>());
    let font = api
      .file(&format!("{FONT}?rand={rand}"), &format!("{WWW}/interns"))
      .await?;
    let glyphs = Self::read(&font)
      .ok_or_else(|| Error::upstream("Shixiseng's font for its search cards could not be read"))?;
    api.ctx.store.cache_put(CACHE, &glyphs.to_cache());
    Ok(glyphs)
  }

  /// The map of a font: each private character of its `cmap` whose glyph
  /// is named after a character.
  fn read(font: &[u8]) -> Option<Self> {
    let cmap = table(font, b"cmap")?;
    let post = table(font, b"post")?;
    let names = custom_names(&post).unwrap_or_default();
    let cmap = ttf_parser::cmap::Table::parse(&cmap)?;
    let post = ttf_parser::post::Table::parse(&post)?;
    let name = |glyph: ttf_parser::GlyphId| {
      names
        .get(usize::from(glyph.0))
        .copied()
        .flatten()
        .or_else(|| post.glyph_name(glyph))
    };
    let mut map = BTreeMap::new();
    for subtable in cmap.subtables {
      if !subtable.is_unicode() {
        continue;
      }
      subtable.codepoints(|code| {
        let ch = subtable.glyph_index(code).and_then(name).and_then(named);
        if let Some(ch) = ch.filter(|_| PRIVATE.contains(&code)) {
          map.insert(code, ch);
        }
      });
    }
    (!map.is_empty()).then_some(Self(map))
  }

  fn from_cache(map: &BTreeMap<String, String>) -> Option<Self> {
    let glyphs: BTreeMap<u32, char> = map
      .iter()
      .filter_map(|(code, ch)| {
        let code = u32::from_str_radix(code, 16).ok()?;
        let mut chars = ch.chars();
        Some((code, chars.next()?))
      })
      .collect();
    (!glyphs.is_empty()).then_some(Self(glyphs))
  }

  fn to_cache(&self) -> BTreeMap<String, String> {
    self
      .0
      .iter()
      .map(|(code, ch)| (format!("{code:x}"), ch.to_string()))
      .collect()
  }

  /// What `code` stands for: the font's character for a private one, the
  /// character itself otherwise.
  fn char(&self, code: u32) -> Option<char> {
    match self.0.get(&code) {
      Some(ch) => Some(*ch),
      None if PRIVATE.contains(&code) => None,
      None => char::from_u32(code),
    }
  }

  /// `text` with its character references (`&#xe942`, with or without the
  /// `;`) and private characters replaced; references the font does not
  /// name are kept.
  pub fn decode(&self, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("&#x") {
      out.push_str(&rest[..at]);
      let after = &rest[at + 3..];
      let digits = after.bytes().take_while(u8::is_ascii_hexdigit).count();
      let code = u32::from_str_radix(&after[..digits], 16).ok();
      match code.and_then(|code| self.char(code)) {
        Some(ch) => {
          out.push(ch);
          let tail = &after[digits..];
          rest = tail.strip_prefix(';').unwrap_or(tail);
        }
        None => {
          out.push_str("&#x");
          rest = after;
        }
      }
    }
    out.push_str(rest);
    out
      .chars()
      .map(|ch| self.0.get(&(ch as u32)).copied().unwrap_or(ch))
      .collect()
  }

  /// `v` with every string decoded.
  pub fn decode_value(&self, v: &Value) -> Value {
    match v {
      Value::String(text) => Value::String(self.decode(text)),
      Value::Array(items) => Value::Array(items.iter().map(|i| self.decode_value(i)).collect()),
      Value::Object(map) => Value::Object(
        map
          .iter()
          .map(|(key, value)| (key.clone(), self.decode_value(value)))
          .collect(),
      ),
      other => other.clone(),
    }
  }
}

/// The character a glyph is named after: `uni6a`, `uni4E00`, `u1F600`, or
/// a name of one character (`x`).
fn named(name: &str) -> Option<char> {
  let hex = name
    .strip_prefix("uni")
    .or_else(|| name.strip_prefix('u'))
    .filter(|hex| (1..=6).contains(&hex.len()));
  if let Some(ch) = hex
    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
    .and_then(char::from_u32)
  {
    return Some(ch);
  }
  let mut chars = name.chars();
  match (chars.next(), chars.next()) {
    (Some(ch), None) => Some(ch),
    _ => None,
  }
}

/// The names a `post` table of version 2.0 gives its glyphs beyond the
/// standard Macintosh ones, by glyph. ttf-parser stops reading them at the
/// first empty name, and this font names its glyph 0 with an empty one.
fn custom_names(post: &[u8]) -> Option<Vec<Option<&str>>> {
  let be16 = |at: usize| Some(u16::from_be_bytes([*post.get(at)?, *post.get(at + 1)?]));
  if post.get(..4)? != [0, 2, 0, 0] {
    return None;
  }
  let glyphs = usize::from(be16(32)?);
  let indexes: Vec<u16> = (0..glyphs)
    .map(|glyph| be16(34 + 2 * glyph))
    .collect::<Option<_>>()?;
  let mut strings = Vec::new();
  let mut at = 34 + 2 * glyphs;
  while let Some(&len) = post.get(at) {
    let name = post.get(at + 1..at + 1 + usize::from(len))?;
    strings.push(std::str::from_utf8(name).ok().filter(|n| !n.is_empty()));
    at += 1 + usize::from(len);
  }
  let custom = |index: u16| {
    let at = usize::from(index).checked_sub(STANDARD_NAMES)?;
    strings.get(at).copied().flatten()
  };
  Some(indexes.into_iter().map(custom).collect())
}

/// A table of a WOFF font (tables compressed with zlib one by one) or of
/// a plain TrueType / OpenType font.
fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<Cow<'a, [u8]>> {
  if !font.starts_with(b"wOFF") {
    let face = ttf_parser::RawFace::parse(font, 0).ok()?;
    return face
      .table(ttf_parser::Tag::from_bytes(tag))
      .map(Cow::Borrowed);
  }
  // The header is 44 bytes, the number of tables at 12; then 20 bytes a
  // table: tag, offset, compressed length, length, checksum.
  let count = u16::from_be_bytes([*font.get(12)?, *font.get(13)?]) as usize;
  let entry = (0..count)
    .filter_map(|i| font.get(44 + 20 * i..64 + 20 * i))
    .find(|entry| &entry[..4] == tag)?;
  let number = |at: usize| {
    u32::from_be_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]]) as usize
  };
  let (offset, packed, size) = (number(4), number(8), number(12));
  let data = font.get(offset..offset.checked_add(packed)?)?;
  if packed >= size {
    return Some(Cow::Borrowed(data));
  }
  let mut out = Vec::with_capacity(size);
  flate2::read::ZlibDecoder::new(data)
    .read_to_end(&mut out)
    .ok()?;
  Some(Cow::Owned(out))
}
