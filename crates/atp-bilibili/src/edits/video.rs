//! Changing a published video (稿件) as the creator center's edit page
//! does (member.bilibili.com/york/videoup, bundle `index.*.js`:
//! `getArchiveInfo`, `getBasicParams`, `getSubmitParams`, `editArchive`).
//! The page loads the whole archive (`archive/view`) and sends the whole
//! record back through `vu/web/edit`, not a difference: every field below
//! is copied from what was loaded, and only the asked ones change. A new
//! cover goes up through `cover/up` first; a new file for a part (换源)
//! through [`uploads`], and the part then names it (`filename`, `cid`).
//! Bilibili reviews the video again after a change. Chapters are not in
//! the archive: they go through the player editor ([`chapters`]), and a
//! change of chapters alone is not reviewed again.

use atp_core::chapters::summary;
use atp_core::cli::Change;
use atp_core::{Action, Edit, Error, ErrorCode, Result, Value, ValueExt};

use super::record::{Fields, body, truthy};
use crate::api::Api;
use crate::chapters;
use crate::refs::Video;
use crate::uploads;

const VIEW: &str = "https://member.bilibili.com/x/vupre/web/archive/view";
const EDIT: &str = "https://member.bilibili.com/x/vu/web/edit";
const MEMBER: &str = "https://member.bilibili.com";
const PAGE: &str = "https://member.bilibili.com/platform/upload/video/frame";
const TITLE_MAX: usize = 80;
const TEXT_MAX: usize = 2000;
const TAGS_MAX: usize = 10;

/// Change one of the account's videos.
pub async fn edit(api: &Api, v: &Video, edit: &Edit) -> Result<Action> {
  api.require_login()?;
  if edit.video.is_some() && edit.chapters.is_some() {
    return Err(Error::input(
      "give --video and --chapters in two edits: the chapters go on the new file once Bilibili has reviewed it",
    ));
  }
  if let Some(path) = &edit.video {
    uploads::check_format(path)?;
  }
  let referer = format!("{PAGE}?type=edit&bvid={}", v.bvid);
  let view = api
    .get(VIEW)
    .arg("bvid", &v.bvid)
    .arg("topic_grey", 1)
    .header("referer", referer.clone())
    .send()
    .await?;
  let archive = changes_archive(edit);
  refuse(&view, edit, archive)?;
  let part = part(&view, v, edit)?;
  let chapters = match &edit.chapters {
    Some(list) => {
      let at = &view.list("videos")[part];
      Some(chapters::change(api, view.at("archive"), at, list).await?)
    }
    None => None,
  };
  let old = fields(view.at("archive"));
  let new = change(view.at("archive"), &old, edit)?;
  ask(v, (&old, &new), edit, part, chapters.as_ref())?;
  if archive {
    send(api, (&view, &new), edit, part, referer).await?;
  }
  if let Some(change) = &chapters {
    chapters::submit(api, change, &v.bvid).await?;
  }
  Ok(
    Action::done("edit", &v.bvid)
      .with_id(&v.bvid)
      .with_url(v.url()),
  )
}

/// Upload what the edit brings (a file for `part`, a cover), then send
/// the record.
async fn send(
  api: &Api,
  (view, new): (&Value, &Fields),
  edit: &Edit,
  part: usize,
  referer: String,
) -> Result<()> {
  let file = match &edit.video {
    Some(path) => Some(uploads::video(api, path).await?),
    None => None,
  };
  let cover = match &edit.cover {
    Some(path) => Some(uploads::cover(api, path, &referer).await?),
    None => None,
  };
  let replaced = file.as_ref().map(|f| (part, f));
  let body = body(view, new, cover, replaced, &api.csrf()?);
  api
    .post(EDIT)
    .json(body)
    .header("referer", referer)
    .header("origin", MEMBER)
    .send()
    .await?;
  Ok(())
}

/// Whether the edit changes the archive itself (and so is reviewed
/// again), not only chapters.
fn changes_archive(edit: &Edit) -> bool {
  edit.text.is_some()
    || edit.title.is_some()
    || edit.topics.is_some()
    || edit.cover.is_some()
    || edit.visibility.is_some()
    || edit.video.is_some()
}

/// The part (counting from 0) that `--part`, or the link's `?p=`, names;
/// a new file or chapters for a video in several parts need one.
fn part(view: &Value, v: &Video, edit: &Edit) -> Result<usize> {
  let count = view.list("videos").len();
  let named = edit.part.or((v.page > 1).then_some(v.page));
  let needed = edit.video.is_some() || edit.chapters.is_some();
  match named {
    Some(n) if (1..=count).contains(&n) => Ok(n - 1),
    Some(n) => Err(Error::input(format!(
      "the video has {count} parts, not {n}"
    ))),
    None if count > 1 && needed => Err(Error::input(format!(
      "the video has {count} parts: say which one with --part N"
    ))),
    None => Ok(0),
  }
}

/// Videos whose record holds what ATP does not send back (staff,
/// charging, commercial orders ...), and edits Bilibili does not take now;
/// only the first check when the archive itself does not change.
fn refuse(view: &Value, edit: &Edit, archive: bool) -> Result<()> {
  let a = view.at("archive");
  if a.u64("aid").is_none() || view.list("videos").is_empty() {
    return Err(Error::not_found(
      "no such video among this account's uploads",
    ));
  }
  if !archive {
    return Ok(());
  }
  let ordered =
    truthy(a.at("adorder_id")) || truthy(a.at("porder.id")) || truthy(a.at("porder.flow_id"));
  let kind = if !view.list("staffs").is_empty() {
    Some("joint submissions (联合投稿)")
  } else if truthy(a.at("charging_pay")) {
    Some("videos for chargers only (充电专属)")
  } else if a.i64("interactive") == Some(1) {
    Some("interactive videos (互动视频)")
  } else if ordered {
    Some("videos with a commercial order (商单)")
  } else {
    None
  };
  if let Some(kind) = kind {
    return Err(Error::unsupported(format!("edit of {kind}")));
  }
  let reason = if truthy(a.at("political_media")) && !truthy(a.at("political_editable")) {
    Some("the video cannot change before it opens (开放浏览前您的稿件暂无法编辑)")
  } else if truthy(a.at("attrs.is_premiere")) && !truthy(a.at("attrs.is_played")) {
    Some("a premiere cannot change before it has played (首映结束之后才可以编辑)")
  } else if edit.cover.is_some() && !a.list("ab_cover_info.ab_covers").is_empty() {
    Some("a cover test is running (封面测试中，暂不支持编辑封面)")
  } else {
    None
  };
  match reason {
    Some(reason) => Err(Error::new(
      ErrorCode::PermissionDenied,
      format!("Bilibili: {reason}"),
    )),
    None => Ok(()),
  }
}

fn fields(a: &Value) -> Fields {
  let own: Vec<String> = a
    .str("tag")
    .unwrap_or_default()
    .split(',')
    .map(str::to_owned)
    .collect();
  Fields {
    title: a.str("title").unwrap_or_default(),
    text: a.str("desc").unwrap_or_default(),
    tags: tags(a, &own),
    private: a.i64("is_only_self") == Some(1),
  }
}

/// The tags as the edit page lists them: the joined topic's (参与话题)
/// first, then `own`, without repeats.
fn tags(a: &Value, own: &[String]) -> Vec<String> {
  let mut tags: Vec<String> = Vec::new();
  let topic = a.str("topic_name");
  for tag in topic.iter().chain(own) {
    let tag = tag.trim().trim_start_matches('#').trim();
    if !tag.is_empty() && !tags.iter().any(|t| t == tag) {
      tags.push(tag.to_owned());
    }
  }
  tags
}

/// The fields after the edit; what is not given stays as it was.
fn change(a: &Value, old: &Fields, edit: &Edit) -> Result<Fields> {
  let title = edit
    .title
    .as_deref()
    .map_or_else(|| old.title.clone(), |t| t.trim().to_owned());
  let tags = match edit.topics.as_deref() {
    None => old.tags.clone(),
    Some([]) => return Err(Error::input("a Bilibili video keeps at least one tag")),
    Some(given) => tags(a, given),
  };
  let private = match edit.visibility.as_deref() {
    None => old.private,
    Some("private") => true,
    Some("public") => false,
    Some(other) => {
      return Err(Error::unsupported(format!(
        "edit --visibility {other} (Bilibili videos are public or private)"
      )));
    }
  };
  let new = Fields {
    title,
    text: edit.text.clone().unwrap_or_else(|| old.text.clone()),
    tags,
    private,
  };
  if new.title.is_empty() {
    return Err(Error::input("a Bilibili video needs a title"));
  }
  if new.title.chars().count() > TITLE_MAX {
    return Err(Error::input(format!(
      "a Bilibili video's title takes at most {TITLE_MAX} characters"
    )));
  }
  if new.text.chars().count() > TEXT_MAX {
    return Err(Error::input(format!(
      "a Bilibili video's description takes at most {TEXT_MAX} characters"
    )));
  }
  if new.tags.len() > TAGS_MAX {
    return Err(Error::input(format!(
      "a Bilibili video takes at most {TAGS_MAX} tags (its topic's included)"
    )));
  }
  Ok(new)
}

/// Show the change and ask (or take `--yes`).
fn ask(
  v: &Video,
  (old, new): (&Fields, &Fields),
  edit: &Edit,
  part: usize,
  chapters: Option<&chapters::Change>,
) -> Result<()> {
  let visibility = |private: bool| if private { "private" } else { "public" };
  let mut change = Change::new(format!("Bilibili video {}:", v.bvid));
  change
    .field("title", &old.title, &new.title)
    .field("text", &old.text, &new.text)
    .field("tags", &old.tags.join(", "), &new.tags.join(", "))
    .field(
      "visibility",
      visibility(old.private),
      visibility(new.private),
    );
  if let Some(path) = &edit.cover {
    change.row("cover", &path.display().to_string());
  }
  if let Some(c) = chapters {
    change.field("chapters", &summary(&c.old), &summary(&c.new));
  }
  if let Some(path) = &edit.video {
    change.row(
      "video",
      &format!("{} replaces part {}", path.display(), part + 1),
    );
    change.row(
      "resets",
      "the part's chapters and player cards are to set again; its danmaku stay with the old file",
    );
  }
  if changes_archive(edit) {
    change.row("review", "Bilibili reviews the video again");
  }
  change.confirm(edit.yes, "change this video on Bilibili")
}
