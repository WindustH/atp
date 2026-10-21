//! Changing a published post as TikTok Studio does it: its edit page loads
//! the post (`/api/v1/post/detail/`, with what each field allows now) and
//! sends what changed through `/tiktok/post/edit/v1/` (scene 1, the web
//! app): the caption with its hashtags, a photo post's title, who can see
//! it. That call goes through Studio's page ([`Browser`]); the post is read
//! like the other API calls. Studio offers no new video, and its cover
//! editor is not reproduced. Nothing goes out before the person has seen
//! the change and said yes; a dry run shows the change, what Studio
//! allows now and the request, and sends nothing.

use std::collections::HashMap;

use atp_core::cli::Change;
use atp_core::output::note;
use atp_core::{Action, Edit, Error, ErrorCode, Result, Value, ValueExt, json};

use super::caption::{self, Caption, Mention};
use super::draft::TITLE_MAX;
use super::{tz_name, visibility_code, visibility_name};
use crate::api::Api;
use crate::browser::{Browser, CONTENT_PAGE, check};
use crate::refs;

const DETAIL: &str = "/api/v1/post/detail/";
const EDIT: &str = "/tiktok/post/edit/v1/";
/// `scene` of an edit from the web app.
const SCENE_WEBAPP: i64 = 1;
/// `biz_type`s of the fields an edit may change.
const BIZ_TEXT: i64 = 0;
const BIZ_VISIBILITY: i64 = 11;
/// `biz_status` of a field the page lets change.
const STATUS_SHOW: i64 = 1;

/// A published post as the edit page sees it.
struct Item {
  id: String,
  photo: bool,
  text: String,
  mentions: Vec<Mention>,
  title: String,
  /// 0 everyone, 1 only you, 2 friends.
  visibility: i64,
  /// `biz_type` → (`biz_status`, `biz_reason`); empty when not told.
  allowed: HashMap<i64, (i64, i64)>,
}

/// What the edit changes; `None` keeps a field.
#[derive(Default)]
struct Plan {
  caption: Option<Caption>,
  title: Option<String>,
  visibility: Option<i64>,
}

impl Plan {
  fn is_empty(&self) -> bool {
    self.caption.is_none() && self.title.is_none() && self.visibility.is_none()
  }
}

/// Change one of the account's posts.
pub async fn edit(api: &Api, post: &str, edit: &Edit) -> Result<Action> {
  api.require_login()?;
  let id = refs::post_ref(api, post).await?.id;
  let v = api
    .get(DETAIL, &[("item_id", id.clone()), ("tz_name", tz_name())])
    .await?;
  let item = Item::of(&id, &v)?;
  let plan = plan(&item, edit)?;
  let refused = refusals(&item, &plan);
  let mut change = summary(&item, &plan);
  let url = item.url();
  if edit.dry_run {
    for (label, value) in state(&item) {
      change.row(label, &value);
    }
    for e in &refused {
      change.row("refused", &e.message);
    }
    change.show();
    if !plan.is_empty() {
      show_request(&request(&item, &plan));
    }
    let message = if refused.is_empty() {
      "edit --dry-run: nothing was sent"
    } else {
      "edit --dry-run: TikTok would refuse this edit (see `refused`); nothing was sent"
    };
    return Ok(
      Action::done("dry-run", &id)
        .with_id(&id)
        .with_url(url)
        .with_message(message),
    );
  }
  if plan.is_empty() {
    return Err(Error::input("nothing to change: the post already reads so"));
  }
  if let Some(e) = refused.into_iter().next() {
    return Err(e);
  }
  change.confirm(edit.yes, "change this post on TikTok")?;
  let browser = Browser::open(api, CONTENT_PAGE).await?;
  api.write_pause().await;
  let v = browser.post(EDIT, &[], &request(&item, &plan)).await?;
  check(EDIT, v)?;
  Ok(Action::done("edit", &id).with_id(&id).with_url(url))
}

impl Item {
  fn of(id: &str, v: &Value) -> Result<Self> {
    // The page reads the post at the top of the answer; some answers nest it.
    let v = if v.at("web_video_param").is_null() && v.at("data").is_object() {
      v.at("data")
    } else {
      v
    };
    if v.at("web_video_param").is_null() && v.at("media_draft").is_null() {
      return Err(Error::not_found(format!(
        "TikTok Studio has no post {id} of yours to edit"
      )));
    }
    let (text, extra) = loaded_text(v);
    let allowed = v
      .list("edit_post_info.edit_post_permission.biz_permissions")
      .iter()
      .filter_map(|p| {
        let status = (p.i64("biz_status")?, p.i64("biz_reason").unwrap_or(0));
        Some((p.i64("biz_type")?, status))
      })
      .collect();
    let images = v.at("media_draft.image_post_content");
    Ok(Self {
      id: id.to_owned(),
      photo: images.is_object(),
      mentions: caption::mentions(&text, &extra),
      text,
      title: images.str("title").unwrap_or_default(),
      visibility: v
        .i64("web_feature_common_info.privacy_setting_info.visibility_type")
        .unwrap_or(0),
      allowed,
    })
  }

  fn url(&self) -> String {
    refs::video_url("", &self.id, self.photo)
  }
}

/// The caption as the edit page loads it: the text of the last edit, else
/// the post's; a caption in lines (`content_desc`) is joined, its entries'
/// offsets made absolute.
fn loaded_text(v: &Value) -> (String, Vec<Value>) {
  let edited = v.at("edit_post_info.edit_post_info.text_edit_info");
  let info = if edited.is_object() {
    edited
  } else {
    v.at("web_video_param.0.single_post_feature_info")
  };
  let lines: Vec<String> = info
    .str("content_desc")
    .filter(|d| !d.is_empty())
    .and_then(|d| serde_json::from_str(&d).ok())
    .unwrap_or_default();
  if lines.is_empty() {
    return (
      info.str("text").unwrap_or_default(),
      info.list("text_extra").to_vec(),
    );
  }
  let starts: Vec<u64> = lines
    .iter()
    .scan(0u64, |at, line| {
      let start = *at;
      *at += caption::utf16(line) as u64 + 1;
      Some(start)
    })
    .collect();
  let extra = info
    .list("content_desc_extra")
    .iter()
    .map(|e| {
      let line = e.u64("line_idx").unwrap_or(0) as usize;
      let base = starts.get(line).copied().unwrap_or(0);
      let mut e = e.clone();
      for key in ["start", "end"] {
        e[key] = (e.u64(key).unwrap_or(0) + base).into();
      }
      e
    })
    .collect();
  (lines.join("\n"), extra)
}

/// What the edit changes, and what the post does not take.
fn plan(item: &Item, edit: &Edit) -> Result<Plan> {
  let mut plan = Plan::default();
  if edit.text.is_some() || edit.topics.is_some() {
    let body = edit.text.clone().unwrap_or_else(|| item.text.clone());
    let (body, topics) = match &edit.topics {
      Some(list) => (
        caption::without_hashtags(&body),
        caption::topic_names(list)?,
      ),
      None => (body, Vec::new()),
    };
    let new = Caption::new(&body, &topics, &item.mentions);
    new.check()?;
    plan.caption = (new.text != item.text.trim()).then_some(new);
  }
  if let Some(title) = &edit.title {
    if !item.photo {
      return Err(Error::input(
        "a TikTok video has no title; --title is for photo posts",
      ));
    }
    let title = title.trim();
    if caption::utf16(title) > TITLE_MAX {
      return Err(Error::input(format!(
        "a photo post's title takes at most {TITLE_MAX} characters"
      )));
    }
    plan.title = (title != item.title).then(|| title.to_owned());
  }
  if let Some(visibility) = &edit.visibility {
    let code = visibility_code(Some(visibility));
    plan.visibility = (code != item.visibility).then_some(code);
  }
  Ok(plan)
}

/// What Studio refuses now, field by field, as its edit page tells.
fn refusals(item: &Item, plan: &Plan) -> Vec<Error> {
  let mut fields = Vec::new();
  if plan.caption.is_some() || plan.title.is_some() {
    fields.push((BIZ_TEXT, "the caption"));
  }
  if plan.visibility.is_some() {
    fields.push((BIZ_VISIBILITY, "who can see it"));
  }
  fields
    .into_iter()
    .filter_map(|(field, name)| {
      let reason = why_not(item, field)?;
      Some(Error::new(
        ErrorCode::PermissionDenied,
        format!("TikTok: {name} cannot change now: {reason}"),
      ))
    })
    .collect()
}

/// Why a field cannot change now, if Studio says it cannot.
fn why_not(item: &Item, field: i64) -> Option<String> {
  let (status, reason) = *item.allowed.get(&field)?;
  let why = match status {
    STATUS_SHOW => return None,
    0 => "Studio offers no such change for this post".to_owned(),
    2 | 5 => "an earlier edit is still in review".to_owned(),
    3 => "TikTok takes one such edit a day".to_owned(),
    4 => "the time to change it (7 days after posting) is over".to_owned(),
    _ => format!("Studio has it grayed out (status {status}, reason {reason})"),
  };
  Some(why)
}

/// `old → new` per field.
fn summary(item: &Item, plan: &Plan) -> Change {
  let kind = if item.photo { "photo post" } else { "video" };
  let mut change = Change::new(format!("TikTok {kind} {}:", item.id));
  if let Some(new) = &plan.caption {
    change.field("caption", &item.text, &new.text);
    let tags: Vec<String> = new.hashtags().iter().map(|t| format!("#{t}")).collect();
    change.row("hashtags", &tags.join(" "));
  }
  if let Some(title) = &plan.title {
    change.field("title", &item.title, title);
  }
  if let Some(code) = plan.visibility {
    change.field(
      "visibility",
      visibility_name(item.visibility),
      visibility_name(code),
    );
  }
  if plan.is_empty() {
    change.row("change", "none: the post already reads so");
  }
  change
}

/// A dry run's rows on what Studio lets change now.
fn state(item: &Item) -> Vec<(&'static str, String)> {
  let row = |field| why_not(item, field).unwrap_or_else(|| "can change".into());
  let mut rows = vec![
    ("caption", row(BIZ_TEXT)),
    ("visibility", row(BIZ_VISIBILITY)),
  ];
  if item.allowed.is_empty() {
    rows.push(("", "TikTok sent no edit permissions with the post".into()));
  }
  rows.push(("url", item.url()));
  rows
}

/// The edit page's request: the time zone, the post, what changed.
fn request(item: &Item, plan: &Plan) -> Value {
  let mut body = json!({
    "tz_name": tz_name(),
    "aweme_id": item.id,
    "scene": SCENE_WEBAPP,
  });
  if let Some(c) = &plan.caption {
    body["is_text_changed"] = true.into();
    body["text"] = c.text.clone().into();
    body["text_extra"] = c.text_extra.clone().into();
    body["markup_text"] = c.markup.clone().into();
  }
  if let Some(title) = &plan.title {
    body["is_title_changed"] = true.into();
    body["title"] = title.clone().into();
  }
  if let Some(code) = plan.visibility {
    body["visibility"] = json!({ "visibility": code });
  }
  body
}

/// A request a dry run would make.
fn show_request(body: &Value) {
  let pretty = serde_json::to_string_pretty(body).unwrap_or_default();
  note(&format!(
    "would POST {EDIT} through TikTok Studio's page:\n{pretty}"
  ));
}
