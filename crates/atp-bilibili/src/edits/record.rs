//! The record `vu/web/edit` takes: the archive as the creator center
//! loaded it, with the fields an edit changes. The edit page also sends
//! every setting of the upload page; Bilibili answered such records from
//! ATP 21001, so the settings at their defaults stay out, and those the
//! video changed from the defaults go back as loaded, so that an edit
//! keeps them.

use atp_core::{Value, ValueExt, json};

use crate::uploads::{Uploaded, https};

/// `copyright` of a reposted (转载) video.
const REPRINT: i64 = 2;
/// `creation_statement` of a video that needs no mark (内容无需标注), and
/// the one the page derives for a reposted video.
const NO_MARK: i64 = -1;
const REPRINT_MARK: i64 = -2;

/// What an edit may change on a video.
#[derive(Debug, Clone)]
pub(super) struct Fields {
  pub title: String,
  /// The description (简介).
  pub text: String,
  pub tags: Vec<String>,
  /// Only the uploader sees it (`is_only_self`).
  pub private: bool,
}

/// The record `vu/web/edit` takes: the archive as loaded, with the new
/// fields and covers (16:9, 4:3), and a new file in place of a part's.
pub(super) fn body(
  view: &Value,
  new: &Fields,
  (cover, cover43): (Option<String>, Option<String>),
  replaced: Option<(usize, &Uploaded)>,
  csrf: &str,
) -> Value {
  let a = view.at("archive");
  let cover = cover.unwrap_or_else(|| https(&a.str("cover").unwrap_or_default()));
  let mut body = json!({
    "aid": a.at("aid"),
    "cover": cover,
    "title": new.title,
    "copyright": a.at("copyright"),
    "creation_statement": { "id": statement(a) },
    "tid": a.at("tid"),
    "tag": new.tags.join(","),
    "desc": new.text,
    "recreate": recreate(a),
    "interactive": 0,
    "videos": videos(view, replaced),
    "new_web_edit": 1,
    "handle_staff": false,
    "topic_grey": 1,
  });
  own(a, new, &mut body);
  kept(view, new, cover43, &mut body);
  body["csrf"] = csrf.into();
  body
}

/// Fields of the archive the edit page sends when it has them.
fn own(a: &Value, new: &Fields, body: &mut Value) {
  if a.i64("copyright") == Some(REPRINT) {
    body["source"] = a.str("source").unwrap_or_default().into();
  }
  if !a.at("human_type2").is_null() {
    body["human_type2"] = a.at("human_type2").clone();
  }
  if let Some(mission) = a.i64("mission_id").filter(|m| *m != 0) {
    body["mission_id"] = mission.into();
  }
  if let Some(topic) = a.u64("topic_id").filter(|t| *t != 0) {
    let mut detail = json!({ "from_topic_id": topic });
    if let Some(from) = a.str("topic_from") {
      detail["from_source"] = format!("arc.web.{from}").into();
    }
    body["topic_id"] = topic.into();
    body["topic_detail"] = detail;
  }
  if let Some(format) = a.i64("desc_format_id").filter(|f| *f != 0) {
    body["desc_format_id"] = format.into();
  }
  // With @-mentions the page sends the parts too; a new text has none.
  if a.str("desc").unwrap_or_default() == new.text && mentions(a.at("desc_v2")) {
    body["desc_v2"] = a.at("desc_v2").clone();
  }
  if let Some(dtime) = a.i64("dtime").filter(|d| *d > 0) {
    body["dtime"] = dtime.into();
  }
  if let Some(dynamic) = a.str("dynamic") {
    body["dynamic"] = dynamic.into();
    if mentions(a.at("dynamic_v2")) {
      body["dynamic_v2"] = a.at("dynamic_v2").clone();
    }
  }
  if let Some(no_reprint) = a.i64("no_reprint") {
    body["no_reprint"] = no_reprint.into();
  }
}

/// The settings the video changed from the defaults, as loaded, and the
/// 4:3 cover: `cover43` when uploaded, else the video's own; and the
/// visibility when it is or becomes private.
fn kept(view: &Value, new: &Fields, cover43: Option<String>, body: &mut Value) {
  let a = view.at("archive");
  // The page's cover editor holds both covers and keeps the 4:3 one when
  // only the 16:9 one changes.
  if let Some(cover43) = cover43.or_else(|| a.str("cover43").map(|c| https(&c))) {
    body["cover43"] = cover43.into();
  }
  if !a.list("ab_cover_info.ab_covers").is_empty() {
    body["is_ab_cover"] = true.into();
    body["ab_cover_info"] = a.at("ab_cover_info").clone();
  }
  if new.private || a.i64("is_only_self") == Some(1) {
    body["is_only_self"] = i32::from(new.private).into();
  }
  // 1 hides the video from the space, 2 (the default) shows it.
  if view.i64("space_hidden").or_else(|| a.i64("space_hidden")) == Some(1) {
    body["space_hidden"] = 1.into();
  }
  if a.i64("copyright") != Some(REPRINT) && view.i64("watermark.state") == Some(1) {
    body["watermark"] = json!({ "state": 1 });
  }
  if a.i64("is_360") == Some(1) {
    body["is_360"] = 1.into();
  }
  if a.i64("is_dolby") == Some(1) {
    body["dolby"] = 1.into();
  }
  if a.i64("lossless_music") == Some(1) {
    body["lossless_music"] = 1.into();
  }
}

/// The archive's creator's statement (创作声明), which the edit page
/// cannot change; without one, none (-1), or the repost mark (-2) on a
/// reposted video as the page derives it.
fn statement(a: &Value) -> i64 {
  match a.i64("creation_statement.id") {
    Some(id) if id != 0 => id,
    _ if a.i64("copyright") == Some(REPRINT) => REPRINT_MARK,
    _ => NO_MARK,
  }
}

/// The parts (分P) as loaded, in their order; a replaced one names its
/// new file and that file's cid, as the edit page does after an upload.
fn videos(view: &Value, replaced: Option<(usize, &Uploaded)>) -> Vec<Value> {
  view
    .list("videos")
    .iter()
    .enumerate()
    .map(|(i, part)| {
      let (filename, cid) = match replaced {
        Some((at, file)) if at == i => (file.filename.clone(), file.cid),
        _ => (
          part.str("filename").unwrap_or_default(),
          part.u64("cid").unwrap_or(0),
        ),
      };
      json!({
        "filename": filename,
        "title": part.str("title").unwrap_or_default(),
        "desc": "",
        "cid": cid,
      })
    })
    .collect()
}

/// `recreate.switch` (or a plain `recreate`) 1 stays 1; anything else is -1.
fn recreate(a: &Value) -> i64 {
  match a.i64("recreate.switch").or_else(|| a.i64("recreate")) {
    Some(1) => 1,
    _ => -1,
  }
}

/// Text parts (`desc_v2`, `dynamic_v2`) holding an @-mention (type 2).
fn mentions(parts: &Value) -> bool {
  parts
    .as_array()
    .is_some_and(|parts| parts.iter().any(|p| p.i64("type") == Some(2)))
}

/// A flag or id that is set: true, a number other than 0, a string other
/// than "" and "0", a non-empty list or object.
pub(super) fn truthy(v: &Value) -> bool {
  match v {
    Value::Bool(b) => *b,
    Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
    Value::String(s) => !s.is_empty() && s != "0",
    Value::Array(a) => !a.is_empty(),
    Value::Object(o) => !o.is_empty(),
    Value::Null => false,
  }
}
