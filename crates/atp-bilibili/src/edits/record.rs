//! The record `vu/web/edit` takes: the whole archive as the creator
//! center loaded it, with the fields an edit changes.

use atp_core::{Value, ValueExt, json};

use crate::uploads::Uploaded;

/// `web_os` of the browser ATP's User-Agent names (macOS): 1 Windows,
/// 2 macOS, 3 others.
const WEB_OS: u8 = 2;
/// `copyright` of a reposted (转载) video.
const REPRINT: i64 = 2;

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
/// fields, and a new file in place of a part's.
pub(super) fn body(
  view: &Value,
  new: &Fields,
  cover: Option<String>,
  replaced: Option<(usize, &Uploaded)>,
  csrf: &str,
) -> Value {
  let a = view.at("archive");
  let watermark = if a.i64("copyright") == Some(REPRINT) {
    0
  } else {
    view.i64("watermark.state").unwrap_or(0)
  };
  // 1 hides the video from the space, 2 (the default) shows it.
  let space_hidden = view
    .i64("space_hidden")
    .or_else(|| a.i64("space_hidden"))
    .filter(|s| *s != 0)
    .unwrap_or(2);
  let mut body = json!({
    "aid": a.at("aid"),
    "title": new.title,
    "copyright": a.at("copyright"),
    "tid": a.at("tid"),
    "tag": new.tags.join(","),
    "mission_id": a.i64("mission_id").unwrap_or(0),
    "desc_format_id": a.i64("desc_format_id").unwrap_or(0),
    "desc": new.text,
    "dynamic": a.str("dynamic").unwrap_or_default(),
    "recreate": recreate(a),
    "interactive": 0,
    "videos": videos(view, replaced),
    "new_web_edit": 1,
    "handle_staff": false,
    "topic_grey": 1,
    "is_only_self": i32::from(new.private),
    "space_hidden": space_hidden,
    "watermark": { "state": watermark },
    "no_reprint": a.i64("no_reprint").unwrap_or(0),
    "is_360": i32::from(a.i64("is_360") == Some(1)),
    "web_os": WEB_OS,
    "csrf": csrf,
  });
  covers(a, cover, &mut body);
  optional(a, new, &mut body);
  body
}

/// The covers: a new one replaces the 16:9 cover and clears the 4:3 crop
/// (the edit page sends none where it has no 4:3 editor either).
fn covers(a: &Value, cover: Option<String>, body: &mut Value) {
  let tested = !a.list("ab_cover_info.ab_covers").is_empty();
  body["is_ab_cover"] = tested.into();
  body["ab_cover_info"] = a.at("ab_cover_info").clone();
  match cover {
    Some(url) => {
      body["cover"] = url.into();
      body["cover43"] = "".into();
      body["ai_cover"] = false.into();
    }
    None => {
      body["cover"] = a.str("cover").unwrap_or_default().into();
      body["cover43"] = a.str("cover43").unwrap_or_default().into();
      body["ai_cover"] = truthy(a.at("ai_cover")).into();
    }
  }
}

/// Fields the edit page sends only when the archive has them.
fn optional(a: &Value, new: &Fields, body: &mut Value) {
  if a.i64("copyright") == Some(REPRINT) {
    body["source"] = a.str("source").unwrap_or_default().into();
  }
  if let Some(id) = a.i64("creation_statement.id") {
    body["creation_statement"] = json!({ "id": id });
  }
  if !a.at("human_type2").is_null() {
    body["human_type2"] = a.at("human_type2").clone();
  }
  if let Some(topic) = a.u64("topic_id").filter(|t| *t != 0) {
    let mut detail = json!({ "from_topic_id": topic });
    if let Some(from) = a.str("topic_from") {
      detail["from_source"] = format!("arc.web.{from}").into();
    }
    body["topic_id"] = topic.into();
    body["topic_detail"] = detail;
  }
  if let Some(dtime) = a.i64("dtime").filter(|d| *d > 0) {
    body["dtime"] = dtime.into();
  }
  // With @-mentions the page sends the parts too; a new text has none.
  if a.str("desc").unwrap_or_default() == new.text && mentions(a.at("desc_v2")) {
    body["desc_v2"] = a.at("desc_v2").clone();
  }
  if mentions(a.at("dynamic_v2")) {
    body["dynamic_v2"] = a.at("dynamic_v2").clone();
  }
  if a.i64("is_dolby") == Some(1) {
    body["dolby"] = 1.into();
  }
  if a.i64("lossless_music") == Some(1) {
    body["lossless_music"] = 1.into();
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
