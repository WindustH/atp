//! New videos (投稿) as the creator center's upload page submits them
//! (member.bilibili.com/york/videoup, bundle `index.*.js`:
//! `getBasicParams`, `getSubmitParams`, `submitArchive`): the file and the
//! cover through [`uploads`], chapters through the player editor's
//! pre-submit ([`chapters`]), then the archive through `add/v3`. The
//! account's settings come from `archive/pre`: which categories (分区) it
//! picks from (the new kind, `type_mode` 2, or the classic `typelist`)
//! and when a scheduled video may go out. Nothing goes out before the
//! person has seen a summary and said yes; Bilibili reviews every video
//! before it shows.

use std::path::Path;

use atp_core::chapters::{self as core_chapters, Chapter};
use atp_core::cli::confirm;
use atp_core::output::note;
use atp_core::{Action, Collection, Draft, Error, Page, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::refs::Video;
use crate::{chapters, uploads};

const PRE: &str = "https://member.bilibili.com/x/vupre/web/archive/pre";
const HUMAN_TYPES: &str = "https://member.bilibili.com/x/vupre/web/archive/human/type2/list";
const ADD: &str = "https://member.bilibili.com/x/vu/web/add/v3";
const MEMBER: &str = "https://member.bilibili.com";
const TITLE_MAX: usize = 80;
const TEXT_MAX: usize = 2000;
const TAGS_MAX: usize = 10;
/// `tid` the page sends with the new kind of categories.
const NEW_KIND_TID: i64 = 21;
/// `web_os` of the browser ATP's User-Agent names (macOS).
const WEB_OS: u8 = 2;

/// What `archive/pre` tells about the account.
struct Pre {
  /// The categories are the new kind (`type_mode` 2): `human_type2`.
  new_kind: bool,
  /// The categories offered: id, name and what they hold.
  categories: Vec<(String, String, String)>,
  /// When a scheduled video may go out, Unix seconds.
  window: Option<(i64, i64)>,
  subtitle: bool,
}

/// What a draft becomes, checked.
struct Plan<'a> {
  video: &'a Path,
  cover: &'a Path,
  title: String,
  tags: Vec<String>,
  category: (String, String),
  chapters: Option<(&'a [Chapter], u32)>,
}

/// The categories a video can go in, as the upload page lists them.
pub async fn categories(api: &Api) -> Result<Page<Collection>> {
  let pre = pre(api).await?;
  let list = pre
    .categories
    .into_iter()
    .map(|(id, name, about)| Collection {
      id,
      kind: "category".into(),
      name,
      description: Some(about).filter(|a| !a.is_empty()),
      ..Collection::default()
    })
    .collect();
  Ok(Page::last(list))
}

/// Publish a video: check the draft, ask, upload, submit.
pub async fn publish(api: &Api, draft: &Draft) -> Result<Action> {
  let partial = check(draft)?;
  api.require_login()?;
  let pre = pre(api).await?;
  let category = category(&pre, draft.category.as_deref().unwrap_or_default())?;
  if let (Some(when), Some((from, to))) = (draft.schedule, pre.window) {
    let at = when.as_second();
    if !(from..=to).contains(&at) {
      return Err(Error::input(format!(
        "Bilibili schedules videos between {} and {}",
        fmt(from),
        fmt(to)
      )));
    }
  }
  let plan = Plan {
    category,
    ..partial
  };
  ask(draft, &plan)?;
  let file = uploads::video(api, plan.video).await?;
  let cover = uploads::cover(api, plan.cover, uploads::PAGE).await?;
  if let Some((list, duration)) = plan.chapters {
    chapters::submit_new(api, file.cid, list, duration).await?;
  }
  let body = body(draft, &plan, &pre, &file, &cover, &api.csrf()?);
  let data = api
    .post(ADD)
    .query("web_location", "333.1024")
    .query("t", jiff::Timestamp::now().as_millisecond())
    .json(body)
    .header("referer", uploads::PAGE)
    .header("origin", MEMBER)
    .send()
    .await?;
  let bvid = data
    .str("bvid")
    .ok_or_else(|| Error::upstream("Bilibili took the video but named no BV id"))?;
  let video = Video::from_bvid(&bvid);
  let url = video.map_or_else(
    || format!("https://www.bilibili.com/video/{bvid}"),
    |v| v.url(),
  );
  Ok(Action::done("post", &bvid).with_id(&bvid).with_url(url))
}

/// What the upload page refuses, before anything is sent; the category
/// is looked up later.
fn check(draft: &Draft) -> Result<Plan<'_>> {
  if draft.reply_to.is_some() || draft.quote.is_some() {
    return Err(Error::unsupported("post --video with --reply-to / --quote"));
  }
  let video = draft.video.as_deref().unwrap_or(Path::new(""));
  uploads::check_format(video)?;
  let title = draft.title.as_deref().unwrap_or_default().trim().to_owned();
  let tags: Vec<String> = draft
    .topics
    .iter()
    .map(|t| t.trim().trim_start_matches('#').trim().to_owned())
    .filter(|t| !t.is_empty())
    .collect();
  let fail = |m: &str| Err(Error::input(m.to_owned()));
  if title.is_empty() || title.chars().count() > TITLE_MAX {
    return fail("a Bilibili video needs a --title of at most 80 characters");
  }
  if draft.text.chars().count() > TEXT_MAX {
    return fail("a Bilibili video's description (TEXT) takes at most 2000 characters");
  }
  if tags.is_empty() || tags.len() > TAGS_MAX {
    return fail("a Bilibili video takes 1 to 10 tags (--topic)");
  }
  if draft.category.is_none() {
    return fail("a Bilibili video needs a --category: `atp bili categories` lists them");
  }
  let Some(cover) = draft.cover.as_deref() else {
    return fail("a Bilibili video needs a cover (--cover, JPEG or PNG)");
  };
  if draft.visibility.as_deref() == Some("friends") {
    return Err(Error::unsupported(
      "post --visibility friends (Bilibili videos are public or private)",
    ));
  }
  Ok(Plan {
    video,
    cover,
    title,
    tags,
    category: Default::default(),
    chapters: video_chapters(&draft.chapters, video)?,
  })
}

/// The chapters with the video's length, checked; the length is read
/// from the file (MP4 or MOV).
fn video_chapters<'a>(list: &'a [Chapter], video: &Path) -> Result<Option<(&'a [Chapter], u32)>> {
  if list.is_empty() {
    return Ok(None);
  }
  let Some(duration) = atp_core::file::video_duration(video) else {
    return Err(Error::input(
      "chapters on a new video need its length, which ATP reads from MP4 and MOV files; \
       add them with `edit --chapters` once the video is published",
    ));
  };
  let duration = duration.floor() as u32;
  core_chapters::check(list, &chapters::RULES, Some(duration))?;
  Ok(Some((list, duration)))
}

/// The account's upload settings.
async fn pre(api: &Api) -> Result<Pre> {
  let v = api.get(PRE).send().await?;
  let new_kind = v.i64("type_mode") == Some(2);
  let categories = if new_kind {
    let types = api.get(HUMAN_TYPES).send().await?;
    types
      .list("type_list")
      .iter()
      .filter_map(|t| {
        let id = t.u64("id")?.to_string();
        Some((id, t.str("name")?, t.str("desc").unwrap_or_default()))
      })
      .collect()
  } else {
    classic(v.list("typelist"))
  };
  let window = v
    .i64("myinfo.deftime")
    .zip(v.i64("myinfo.deftime_end"))
    .filter(|(from, to)| *from > 0 && to > from);
  Ok(Pre {
    new_kind,
    categories,
    window,
    subtitle: v.bool("myinfo.subtitle") == Some(true) || v.i64("myinfo.subtitle") == Some(1),
  })
}

/// The classic categories: the regions' sub-categories, named with
/// their region.
fn classic(regions: &[Value]) -> Vec<(String, String, String)> {
  let mut list = Vec::new();
  for region in regions {
    let parent = region.str("name").unwrap_or_default();
    for child in region.list("children") {
      if let (Some(id), Some(name)) = (child.u64("id"), child.str("name")) {
        let about = child.str("desc").unwrap_or_default();
        list.push((id.to_string(), format!("{parent} · {name}"), about));
      }
    }
  }
  list
}

/// The category given, among those the account picks from.
fn category(pre: &Pre, id: &str) -> Result<(String, String)> {
  let id = id.trim();
  match pre.categories.iter().find(|(c, _, _)| c == id) {
    Some((id, name, _)) => Ok((id.clone(), name.clone())),
    None if pre.categories.is_empty() => Ok((id.to_owned(), String::new())),
    None => Err(Error::input(format!(
      "no Bilibili category {id}: `atp bili categories` lists them"
    ))),
  }
}

/// Show what goes out and ask (or take `--yes`).
fn ask(draft: &Draft, plan: &Plan) -> Result<()> {
  let mut lines = vec!["Bilibili video:".to_owned()];
  let mut line = |label: &str, value: String| lines.push(format!("  {label:<11} {value}"));
  line("title", plan.title.clone());
  if !draft.text.is_empty() {
    line("text", atp_core::text::truncate(&draft.text, 60));
  }
  line("tags", plan.tags.join(", "));
  let (id, name) = &plan.category;
  line("category", format!("{id} {name}").trim().to_owned());
  line(
    "visibility",
    draft.visibility.clone().unwrap_or_else(|| "public".into()),
  );
  if let Some(when) = draft.schedule {
    line("schedule", atp_core::text::fmt_time(when));
  }
  if let Some((list, _)) = plan.chapters {
    line("chapters", core_chapters::summary(list));
  }
  for path in [plan.video, plan.cover] {
    line("file", format!("{} ({})", path.display(), size_of(path)));
  }
  line(
    "review",
    "Bilibili reviews the video before it shows".into(),
  );
  note(&lines.join("\n"));
  confirm(draft.yes, "publish this on Bilibili")
}

/// The record `add/v3` takes, as the upload page builds it for one file.
fn body(
  draft: &Draft,
  plan: &Plan,
  pre: &Pre,
  file: &uploads::Uploaded,
  cover: &str,
  csrf: &str,
) -> Value {
  let category: i64 = plan.category.0.parse().unwrap_or(0);
  let (tid, human_type2) = if pre.new_kind {
    (NEW_KIND_TID, category)
  } else {
    (category, 0)
  };
  let mut body = json!({
    "cover": cover,
    "cover43": "",
    "ai_cover": false,
    "title": plan.title,
    "copyright": 1,
    "human_type2": human_type2,
    "tid": tid,
    "tag": plan.tags.join(","),
    "desc": draft.text,
    "dynamic": "",
    "recreate": 0,
    "interactive": 0,
    "videos": [{ "filename": file.filename, "title": plan.title, "desc": "", "cid": file.cid }],
    "act_reserve_create": 0,
    "act_reserve_create_title": "",
    "no_disturbance": 0,
    "is_only_self": i32::from(draft.visibility.as_deref() == Some("private")),
    "space_hidden": 2,
    "watermark": { "state": 0 },
    "no_reprint": 1,
    "web_os": WEB_OS,
    "csrf": csrf,
  });
  if !pre.new_kind {
    body["desc_format_id"] = 0.into();
  }
  if let Some(when) = draft.schedule {
    body["dtime"] = when.as_second().into();
  }
  if pre.subtitle {
    body["subtitle"] = json!({ "open": 0, "lan": "" });
  }
  if plan.chapters.is_some() {
    body["has_chapter"] = true.into();
  }
  body
}

fn fmt(seconds: i64) -> String {
  atp_core::text::from_secs(seconds).map_or_else(|| seconds.to_string(), atp_core::text::fmt_time)
}

fn size_of(path: &Path) -> String {
  let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
  atp_core::text::fmt_size(bytes)
}
