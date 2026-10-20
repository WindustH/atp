//! Changing a published work as the creator platform does it: the edit
//! page (作品管理 → 修改作品) loads the work (`item/info`) and whether it
//! may still change (`update/desc/valid/v2`), then sends the text, title,
//! hashtags, covers (竖封面 and 横封面) and a new video file (更换视频,
//! the page's `video_id` once it changed) through `update/desc/`; the
//! manage page's permission form sets who can see it (`aweme/update/`)
//! and its chapter editor (编辑章节) the chapters
//! ([`chapters`](super::chapters)). Nothing goes out before the person
//! has seen the change and said yes.

use std::time::Duration;

use atp_core::chapters::Chapter;
use atp_core::cli::Change;
use atp_core::output::note;
use atp_core::{Action, Edit, Error, ErrorCode, Result, Value, ValueExt};

use super::chapters;
use super::draft::{
  IMAGE_TITLE_MAX, TEXT_MAX, VIDEO_TITLE_MAX, chapter_summary, cover_notes, size_of, utf16,
};
use super::{MANAGE_PAGE, picture};
use crate::api::Api;
use crate::creator::Creator;
use crate::items::{self, Covers, Editor, Picture, Tag};
use crate::uploads::Uploader;
use crate::{refs, users};

const ITEM_INFO: &str = "/web/api/media/item/info/";
const EDITABLE: &str = "/web/api/media/update/desc/valid/v2/";
const UPDATE_DESC: &str = "/web/api/media/update/desc/";
const UPDATE_PERMISSION: &str = "/web/api/media/aweme/update/";
const CHAPTER_UPSERT: &str = "/aweme/v1/chapter/upsert";
const CHAPTER_DELETE: &str = "/aweme/v1/chapter/delete/";
const EDIT_PAGE: &str = "https://creator.douyin.com/creator-micro/content/post";

/// Why `media_fields` holds a new video back (`cant_not_edit_reason`):
/// the page asks before going on (bullet comments, chapters, hotspots) or
/// stops (DOU+ delivery, a promotion task or activity still in review).
const MEDIA_DOU_PLUS: i64 = 12;
const MEDIA_DANMAKU: i64 = 13;
const MEDIA_IN_REVIEW: i64 = 14;
const MEDIA_CHAPTERS: i64 = 16;
const MEDIA_HOTSPOTS: i64 = 17;

/// A published work as the edit page sees it.
struct Work {
  id: String,
  kind: Kind,
  title: String,
  editor: Editor,
  /// 0 public, 1 only me, 2 friends (`status.private_status`).
  visibility: i64,
  download: i64,
  /// The video's length in seconds, when `item/info` tells it.
  duration: Option<u32>,
  chapters: Chapters,
}

/// A video's chapters as the manage page's chapter editor loads them.
#[derive(Default)]
struct Chapters {
  /// `None` when `item/info` does not tell them.
  list: Option<Vec<Chapter>>,
  /// The summary (全文总结) and progress-bar setting kept on a change.
  summary: String,
  bar: i64,
  /// Why they cannot change now, when Douyin says so.
  locked: Option<String>,
}

/// What `update/desc/valid/v2` allows.
#[derive(Default)]
struct Editable {
  /// The edits used and allowed (`edited_times`, `max_edit_times`: 5 a
  /// work), when Douyin tells them. Each `update/desc/` uses one; the
  /// manage page's chapter editor and permission form use none.
  edits: Option<(u64, u64)>,
  /// Why the text, title, covers and video cannot change now
  /// (`can_edit` false, its `toast`).
  refused: Option<String>,
  /// A new video: whether the page offers it, may send it, and why not.
  media_offered: bool,
  media_allowed: bool,
  media_reason: i64,
  media_toast: Option<String>,
}

enum Kind {
  Video { vid: String, covers: Covers },
  Images,
}

/// Change one of the account's works.
pub async fn edit(api: &Api, post: &str, edit: &Edit) -> Result<Action> {
  if edit.part.is_some() {
    return Err(Error::input(
      "a Douyin video has one part; leave out --part",
    ));
  }
  if edit.video.is_some() && edit.chapters.is_some() {
    return Err(Error::input(
      "change the chapters once the new video has passed review: Douyin asks for them \
       to be confirmed again after a new video",
    ));
  }
  let id = refs::aweme(api, post).await?;
  let creator = Creator::open(api).await?;
  let info = creator.get(ITEM_INFO).arg("item_id", &id).send().await?;
  let work = Work::of(&id, info.at("aweme"))?;
  check_media(&work, edit)?;
  let words = edit.text.is_some() || edit.title.is_some() || edit.topics.is_some();
  let covers = edit.cover.is_some() || edit.cover_horizontal.is_some();
  let describes = words || covers || edit.video.is_some();
  let visibility = edit.visibility.as_deref().map(visibility_code);
  // The edits left show either way; only `update/desc/` needs one.
  let allowed = match editable(&creator, &id).await {
    Ok(allowed) => allowed,
    Err(e) if describes => return Err(e),
    Err(_) => Editable::default(),
  };
  if describes {
    allowed.check_edits()?;
  }
  if edit.video.is_some() {
    allowed.check_media()?;
  }
  let (title, editor) = change(&work, edit)?;
  let notes = match work.kind {
    Kind::Video { .. } => {
      cover_notes(edit.cover.as_deref(), edit.cover_horizontal.as_deref()).await
    }
    Kind::Images => Vec::new(),
  };
  ask(
    &work,
    edit,
    (&title, &editor),
    (&allowed, describes),
    &notes,
  )?;

  if describes {
    let form = describe(api, &creator, &work, edit, &title, &editor, words).await?;
    pause(api).await;
    creator
      .post_form(UPDATE_DESC, &form)
      .arg("read_aid", 2906)
      .referer(&work.page())
      .send()
      .await?;
  }
  if let Some(list) = &edit.chapters {
    pause(api).await;
    let form = if list.is_empty() {
      vec![("item_id".to_owned(), id.clone())]
    } else {
      let c = &work.chapters;
      items::chapters_upsert(&id, list, &c.summary, c.bar)
    };
    let path = if list.is_empty() {
      CHAPTER_DELETE
    } else {
      CHAPTER_UPSERT
    };
    creator
      .post_form(path, &form)
      .referer(MANAGE_PAGE)
      .send()
      .await?;
  }
  if let Some(code) = visibility.filter(|code| *code != work.visibility) {
    pause(api).await;
    let form = [
      ("item_id", id.clone()),
      ("download", work.download.to_string()),
      ("visibility_type", code.to_string()),
    ]
    .map(|(k, v)| (k.to_owned(), v));
    creator
      .post_form(UPDATE_PERMISSION, &form)
      .referer(MANAGE_PAGE)
      .send()
      .await?;
  }
  Ok(Action::done("edit", &id).with_id(&id).with_url(work.url()))
}

/// What the work refuses before anything is asked: a new video or
/// chapters on an image post, chapters against the chapter editor's rules
/// or while Douyin holds them.
fn check_media(work: &Work, edit: &Edit) -> Result<()> {
  if matches!(work.kind, Kind::Images) && (edit.video.is_some() || edit.chapters.is_some()) {
    return Err(Error::input(
      "an image post has no video or chapters to change",
    ));
  }
  if matches!(work.kind, Kind::Images) && edit.cover_horizontal.is_some() {
    return Err(Error::input(
      "an image post has no 4:3 cover; leave out --cover-horizontal",
    ));
  }
  let Some(list) = &edit.chapters else {
    return Ok(());
  };
  if let Some(reason) = &work.chapters.locked {
    return Err(Error::new(
      ErrorCode::PermissionDenied,
      format!("Douyin: {reason}"),
    ));
  }
  if !list.is_empty() {
    return chapters::check(list, work.duration);
  }
  if work.chapters.list.as_ref().is_some_and(Vec::is_empty) {
    return Err(Error::input("this video has no chapters to remove"));
  }
  Ok(())
}

/// The `update/desc/` form, after uploading the new video and covers.
async fn describe(
  api: &Api,
  creator: &Creator<'_>,
  work: &Work,
  edit: &Edit,
  title: &str,
  editor: &Editor,
  words: bool,
) -> Result<items::Form> {
  let uploads = edit.video.is_some() || edit.cover.is_some() || edit.cover_horizontal.is_some();
  let uploader = if uploads {
    Some(Uploader::new(creator, &users::uid(api).await?).await?)
  } else {
    None
  };
  let mut new_vid = None;
  if let (Some(up), Some(path)) = (&uploader, &edit.video) {
    note("uploading the video");
    new_vid = Some(up.video(path).await?.vid);
  }
  let mut cover = None;
  if let (Some(up), Some(path)) = (&uploader, &edit.cover) {
    note("uploading the cover");
    cover = Some(picture(up.image(path).await?));
  }
  let mut horizontal = None;
  if let (Some(up), Some(path)) = (&uploader, &edit.cover_horizontal) {
    note("uploading the 4:3 cover");
    horizontal = Some(picture(up.image(path).await?));
  }
  Ok(match &work.kind {
    Kind::Video { vid, covers } => {
      let vid = new_vid.as_deref().unwrap_or(vid);
      let (new, horizontal) = (cover.as_ref(), horizontal.as_ref());
      items::video_modify(&work.id, vid, title, editor, covers, new, horizontal)
    }
    Kind::Images => {
      let words = words.then_some((title, editor));
      items::images_modify(&work.id, words, cover.as_ref())
    }
  })
}

impl Editable {
  /// Fails when the work takes no more `update/desc/` edits: Douyin says
  /// so (`can_edit`), or all of them are used.
  fn check_edits(&self) -> Result<()> {
    if let Some(reason) = &self.refused {
      return Err(Error::new(
        ErrorCode::PermissionDenied,
        format!("Douyin: {reason}"),
      ));
    }
    match self.edits {
      Some((used, max)) if used >= max => Err(Error::new(
        ErrorCode::PermissionDenied,
        format!(
          "Douyin: this work has used all {max} of its edits (每个作品最多修改{max}次); its text, \
           title, hashtags, covers and video can no longer change. Chapters and visibility still can"
        ),
      )),
      _ => Ok(()),
    }
  }

  /// The summary's rows on the edits: those left, what this change uses,
  /// and a warning before the last one.
  fn rows(&self, describes: bool) -> Vec<(&'static str, String)> {
    let Some((used, max)) = self.edits else {
      return Vec::new();
    };
    let left = max.saturating_sub(used);
    let mut rows = vec![("edits", format!("{left} of {max} left ({used} used)"))];
    if !describes {
      rows.push((
        "",
        "this change uses none: chapters and visibility go through the manage page".into(),
      ));
    } else if left == 1 {
      rows.push((
        "",
        "this is the work's last edit: give the text, title, hashtags, covers and video \
         together in it (chapters and visibility do not count)"
          .into(),
      ));
    } else {
      rows.push(("", "this change uses 1".into()));
    }
    rows
  }

  /// Fails when Douyin takes no new video for this work now: the page
  /// offers 更换视频 only when `media_fields` shows the button, and sends
  /// one only when it allows it.
  fn check_media(&self) -> Result<()> {
    let refuse = |fallback: &str| {
      let reason = self.media_toast.clone().unwrap_or_else(|| fallback.into());
      Err(Error::new(
        ErrorCode::PermissionDenied,
        format!("Douyin: {reason}"),
      ))
    };
    if !self.media_offered {
      return refuse(
        "this work's video cannot be replaced (the edit page offers no 更换视频 for it)",
      );
    }
    if !self.media_allowed {
      return refuse("this work's video cannot be replaced now");
    }
    match self.media_reason {
      MEDIA_DOU_PLUS => Err(Error::new(
        ErrorCode::PermissionDenied,
        "Douyin: the work is in DOU+ delivery; stop or finish the DOU+ order before replacing its video",
      )),
      MEDIA_IN_REVIEW => Err(Error::new(
        ErrorCode::PermissionDenied,
        "Douyin: the work is still in review for a promotion task or activity; replace its video after that",
      )),
      _ => Ok(()),
    }
  }

  /// What a new video brings along, as the page tells before sending one.
  fn media_notes(&self) -> Vec<&'static str> {
    let mut notes = vec![
      "the new video is reviewed again; once it passes it replaces the old one and the likes, comments and other counts stay",
      "Douyin allows 3 replacements per work, and the new video must not differ much from the old one",
      "a music set on the old video gives way to the new file's own sound",
    ];
    match self.media_reason {
      MEDIA_DANMAKU => notes.push("its bullet comments (弹幕) are cleared within 24 hours"),
      MEDIA_CHAPTERS => {
        notes.push("its chapters must be confirmed again: `edit --chapters FILE` once it passed")
      }
      MEDIA_HOTSPOTS => notes.push("its components (hotspots, labels) are reviewed again"),
      _ => {}
    }
    notes
  }
}

impl Work {
  fn of(id: &str, aweme: &Value) -> Result<Self> {
    if aweme.is_null() {
      return Err(Error::not_found(format!(
        "no work {id} on this account's creator platform"
      )));
    }
    let images = !aweme.list("images").is_empty();
    let vid = first(aweme, &["video.vid", "vid", "video.play_addr.uri"]);
    let (kind, (title, editor)) = match vid {
      _ if images => (Kind::Images, items::image_editor(aweme)?),
      Some(vid) => {
        let covers = covers(aweme);
        tracing::debug!("item/info: vid {vid}, covers {covers:?}");
        (Kind::Video { vid, covers }, items::video_editor(aweme)?)
      }
      None => {
        tracing::debug!("item/info aweme keys: {:?}", keys(aweme));
        return Err(Error::unsupported(
          "edit of this kind of work (only videos and image posts)",
        ));
      }
    };
    Ok(Self {
      id: id.to_owned(),
      kind,
      title,
      editor,
      visibility: aweme.i64("status.private_status").unwrap_or(0),
      download: aweme.i64("video_control.allow_download").unwrap_or(1),
      duration: ["video.duration", "duration"]
        .iter()
        .find_map(|p| aweme.u64(p).filter(|ms| *ms > 0))
        .and_then(|ms| u32::try_from((ms + 500) / 1000).ok()),
      chapters: Chapters::of(aweme),
    })
  }

  fn name(&self) -> &'static str {
    match self.kind {
      Kind::Video { .. } => "video",
      Kind::Images => "image post",
    }
  }

  fn url(&self) -> String {
    match self.kind {
      Kind::Video { .. } => refs::video_url(&self.id),
      Kind::Images => refs::note_url(&self.id),
    }
  }

  /// The edit page: the referer of `update/desc/`.
  fn page(&self) -> String {
    let form = match self.kind {
      Kind::Video { .. } => "video",
      Kind::Images => "image",
    };
    format!("{EDIT_PAGE}/{form}?enter_from=manage&mid={}", self.id)
  }
}

/// The title and editor content after the edit: a new text replaces the
/// body, new topics the hashtags; what is not given stays as it was, and
/// an untouched text keeps its hashtags where they are.
fn change(work: &Work, edit: &Edit) -> Result<(String, Editor)> {
  let title = edit
    .title
    .as_deref()
    .map_or_else(|| work.title.clone(), |t| t.trim().to_owned());
  let editor = if edit.text.is_none() && edit.topics.is_none() {
    work.editor.clone()
  } else {
    let old = work.editor.tags();
    let tags: Vec<Tag> = match &edit.topics {
      Some(names) => names
        .iter()
        .map(|t| t.trim().trim_start_matches('#').trim().to_owned())
        .filter(|name| !name.is_empty())
        .map(|name| {
          let id = old.iter().find(|t| t.name == name).map_or(0, |t| t.id);
          Tag { name, id }
        })
        .collect(),
      None => old,
    };
    let body = edit.text.clone().unwrap_or_else(|| work.editor.body());
    Editor::typed(&body, &tags)
  };
  let max = match work.kind {
    Kind::Video { .. } => VIDEO_TITLE_MAX,
    Kind::Images => IMAGE_TITLE_MAX,
  };
  if utf16(&title) > max {
    return Err(Error::input(format!(
      "a Douyin {}'s title takes at most {max} characters",
      work.name()
    )));
  }
  if utf16(&editor.text) > TEXT_MAX {
    return Err(Error::input("the text takes at most 1000 characters"));
  }
  Ok((title, editor))
}

/// Whether the work's text, title and cover may still change and how many
/// edits are used (`basic_fields`, which the edit page's tip 每个作品最多修改
/// 5次，目前已经使用N次修改机会 reads), and whether its video may be
/// replaced (`media_fields`; taken as allowed when the answer has none,
/// leaving the verdict to `update/desc/`).
async fn editable(creator: &Creator<'_>, id: &str) -> Result<Editable> {
  let v = creator
    .get(EDITABLE)
    .arg("item_id", id)
    .arg("action", "init")
    .send()
    .await?;
  let fields = v.at("basic_fields");
  let refused = (fields.bool("can_edit") == Some(false)).then(|| {
    fields
      .str("toast")
      .unwrap_or_else(|| "this work can no longer be edited".into())
  });
  let media = v.at("media_fields");
  tracing::debug!("update/desc/valid/v2 media_fields: {media}");
  Ok(Editable {
    edits: match (fields.u64("edited_times"), fields.u64("max_edit_times")) {
      (Some(used), Some(max)) if max > 0 => Some((used, max)),
      _ => None,
    },
    refused,
    media_offered: media.bool("show_button").unwrap_or(true),
    media_allowed: media.bool("can_edit").unwrap_or(true),
    media_reason: media.i64("cant_not_edit_reason").unwrap_or(0),
    media_toast: media.str("toast"),
  })
}

impl Chapters {
  /// What the manage page's chapter editor reads off a work: the list
  /// (`chapter_list`), summary (`chapter_abstract`), progress-bar setting
  /// (`chapter_type`, shown unless 0), and what keeps it closed (a lock in
  /// `chapter_data`, chapters in review, smart chapters being made).
  fn of(aweme: &Value) -> Self {
    let list = aweme
      .at("chapter_list")
      .is_array()
      .then(|| chapters::of_work(aweme));
    let locked = if aweme.bool("chapter_data.disable_edit") == Some(true) {
      Some("this video's chapters cannot be changed")
    } else if aweme.i64("chapter_review_status") == Some(1) {
      Some("the chapters last submitted are still in review (章节要点审核中); try later")
    } else if aweme.i64("recommend_chapter_info.generation_status") == Some(0) {
      Some("smart chapters are being made for this video; try later")
    } else {
      None
    };
    Self {
      list,
      summary: aweme.str("chapter_abstract").unwrap_or_default(),
      bar: aweme.i64("chapter_type").map_or(1, |t| i64::from(t == 1)),
      locked: locked.map(str::to_owned),
    }
  }
}

/// Show the change, with `notes` on it and the edits left, and ask (or
/// take `--yes`).
fn ask(
  work: &Work,
  edit: &Edit,
  (title, editor): (&str, &Editor),
  (allowed, describes): (&Editable, bool),
  notes: &[String],
) -> Result<()> {
  let hashtags = |e: &Editor| {
    let names: Vec<String> = e.tags().iter().map(|t| format!("#{}", t.name)).collect();
    names.join(" ")
  };
  let visibility = edit
    .visibility
    .as_deref()
    .map_or(work.visibility, visibility_code);
  let mut change = Change::new(format!("Douyin {} {}:", work.name(), work.id));
  change
    .field("title", &work.title, title)
    .field("text", &work.editor.body(), &editor.body())
    .field("hashtags", &hashtags(&work.editor), &hashtags(editor))
    .field(
      "visibility",
      visibility_name(work.visibility),
      visibility_name(visibility),
    );
  if let Some(new) = &edit.chapters {
    let old = work
      .chapters
      .list
      .as_deref()
      .map_or_else(|| "(not shown)".into(), chapter_summary);
    change.field("chapters", &old, &chapter_summary(new));
    if !new.is_empty() {
      change.row("", "chapters are reviewed before they show");
    }
  }
  if let Some(path) = &edit.cover {
    change.row("cover", &path.display().to_string());
  }
  if let Some(path) = &edit.cover_horizontal {
    change.row("cover 4:3", &path.display().to_string());
  }
  for text in notes {
    change.row("", text);
  }
  if let Some(path) = &edit.video {
    let file = format!("{} ({})", path.display(), size_of(path));
    change.row("video", &format!("{file}, replacing the video"));
    for note in allowed.media_notes() {
      change.row("", note);
    }
    if edit.cover.is_none() {
      change.row(
        "",
        "the cover stays; give --cover for one from the new video",
      );
    }
  }
  for (label, value) in allowed.rows(describes) {
    change.row(label, &value);
  }
  change.confirm(edit.yes, "change this work on Douyin")
}

/// A video's covers as the edit page fills them in: `Cover` (or `cover`),
/// with the video's size when the cover has none, and `horizontal_cover`.
fn covers(aweme: &Value) -> Covers {
  let pick = |paths: &[&str]| first(aweme, paths).unwrap_or_default();
  let size = |key: &str, fallback: &str| {
    aweme
      .u64(&format!("Cover.{key}"))
      .or_else(|| aweme.u64(&format!("cover.{key}")))
      .or_else(|| aweme.u64(fallback))
      .unwrap_or(0)
  };
  let cover = Picture {
    uri: pick(&["Cover.uri", "cover.uri", "video.cover.uri"]),
    width: size("width", "video.width"),
    height: size("height", "video.height"),
  };
  let horizontal = aweme.str("horizontal_cover.uri").map(|uri| Picture {
    uri,
    width: aweme.u64("horizontal_cover.width").unwrap_or(0),
    height: aweme.u64("horizontal_cover.height").unwrap_or(0),
  });
  Covers { cover, horizontal }
}

fn visibility_code(name: &str) -> i64 {
  match name {
    "private" => 1,
    "friends" => 2,
    _ => 0,
  }
}

fn visibility_name(code: i64) -> &'static str {
  match code {
    1 => "private",
    2 => "friends",
    _ => "public",
  }
}

fn first(v: &Value, paths: &[&str]) -> Option<String> {
  paths.iter().find_map(|p| v.str(p))
}

fn keys(v: &Value) -> Vec<&str> {
  v.as_object()
    .map(|o| o.keys().map(String::as_str).collect())
    .unwrap_or_default()
}

async fn pause(api: &Api) {
  api
    .ctx
    .http
    .pause(Duration::from_secs(2), Duration::from_secs(6))
    .await;
}
