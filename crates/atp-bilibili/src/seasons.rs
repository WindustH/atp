//! Your own collections (合集; seasons in Bilibili's API), managed the way
//! the creator center's 合集管理 page manages them
//! (member.bilibili.com/platform/upload-manager/ep, bundle
//! `creativecenter-v3`: its API in chunk 3543, the page in chunk 2452).
//!
//! A season holds sections (分组 / 小节; a single 正片 one unless it uses
//! several), and a section holds episodes in order: one video each, under
//! a title of its own (the video's title when it goes in). Videos go in
//! through `section/episodes/add` (several at once) and out through
//! `section/episode/del` (one at a time); a section's order changes with
//! `section/edit`, which takes the whole list of its episodes. Writes with
//! a JSON body carry `csrf` in the query, form writes in the body, as the
//! page's client sends them. A video can be in one season only: before it
//! lets one in, the page asks `section/episode/verify`, and so does ATP.
//! Bilibili reviews new seasons and their changes.
//!
//! The upload page puts a new video in a season after submitting it, with
//! the same `episodes/add` (`addVideoToSeason`): `add/v3` itself names
//! only paid seasons. So a video goes in once published.

use std::path::Path;

use atp_core::cli::Change;
use atp_core::file::Image;
use atp_core::output::note;
use atp_core::text::{one_line, truncate};
use atp_core::{
  Action, Collection, Error, ErrorCode, Page, PageReq, Post, Result, Value, ValueExt, bulk, json,
};

use crate::api::{Api, Call};
use crate::paging::Pn;
use crate::refs::{VIDEO_URL, Video, space_url};
use crate::{uploads, users};

const MEMBER: &str = "https://member.bilibili.com";
/// The 合集管理 page, the referer of its calls.
const PAGE: &str = "https://member.bilibili.com/platform/upload-manager/ep";
const SEASONS: &str = "https://member.bilibili.com/x2/creative/web/seasons";
const SEASON: &str = "https://member.bilibili.com/x2/creative/web/season";
const REMAIN: &str = "https://member.bilibili.com/x2/creative/web/season/remain";
const ADD: &str = "https://member.bilibili.com/x2/creative/web/season/add";
const EDIT: &str = "https://member.bilibili.com/x2/creative/web/season/edit";
const DEL: &str = "https://member.bilibili.com/x2/creative/web/season/del";
const SECTION: &str = "https://member.bilibili.com/x2/creative/web/season/section";
const SECTION_EDIT: &str = "https://member.bilibili.com/x2/creative/web/season/section/edit";
const EPISODES_ADD: &str =
  "https://member.bilibili.com/x2/creative/web/season/section/episodes/add";
const EPISODE_DEL: &str = "https://member.bilibili.com/x2/creative/web/season/section/episode/del";
const VERIFY: &str = "https://member.bilibili.com/x2/creative/web/season/section/episode/verify";
/// A video's record with its parts (cids), as the page reads it.
const ARCHIVE: &str = "https://member.bilibili.com/x/web/archive/videos";
/// The picture the upload page gives a 合集 created there without a cover.
const DEFAULT_COVER: &str =
  "https://i0.hdslb.com/bfs/templar/york-static/viedeo_material_default.png";
const TITLE_MAX: usize = 50;
const DESC_MAX: usize = 500;
/// Videos one add takes: the page lets 50 be picked at once.
const BATCH: usize = 50;

/// A season's or an episode's state, as the page labels it
/// (`reviewState`, from `state` and `partState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
  Open,
  Review,
  Rejected,
  PartlyRejected,
  Disabled,
  Draft,
  Deleted,
  Private,
  Unknown,
}

impl State {
  fn of(v: &Value) -> Self {
    let part = v.i64("partState").unwrap_or(0);
    match v.i64("state").unwrap_or(0) {
      0 if part == -1 || part == -6 => Self::Review,
      0 if part == -2 => Self::PartlyRejected,
      0 => Self::Open,
      -1 | -6 => Self::Review,
      -2 => Self::Rejected,
      -4 => Self::Disabled,
      -10 => Self::Draft,
      -100 => Self::Deleted,
      _ => Self::Unknown,
    }
  }

  fn name(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::Review => "in review",
      Self::Rejected => "rejected",
      Self::PartlyRejected => "partly rejected",
      Self::Disabled => "disabled",
      Self::Draft => "draft",
      Self::Deleted => "deleted",
      Self::Private => "private",
      Self::Unknown => "unknown",
    }
  }

  fn label(self) -> String {
    let zh = match self {
      Self::Open => "正常显示",
      Self::Review => "正在审核",
      Self::Rejected => "审核打回",
      Self::PartlyRejected => "部分内容被审核打回",
      Self::Disabled => "管理禁用",
      Self::Draft => "草稿",
      Self::Deleted => "删除",
      Self::Private => "仅自见",
      Self::Unknown => "未知",
    };
    format!("{} ({zh})", self.name())
  }
}

struct Season {
  id: u64,
  mid: String,
  title: String,
  desc: String,
  cover: String,
  is_end: i64,
  price: i64,
  paid: bool,
  activity: bool,
  state: State,
  reject: Option<String>,
  sections: Vec<Section>,
}

impl Season {
  /// `{season, sections: {sections}}`, as the list and the detail give it.
  fn parse(v: &Value) -> Option<Self> {
    let s = v.at("season");
    let state = State::of(s);
    let reject = match state {
      State::PartlyRejected => Some("部分内容被审核打回".to_owned()),
      State::Rejected => s.str("rejectReason"),
      _ => None,
    };
    Some(Self {
      id: s.u64("id")?,
      mid: s.str("mid").unwrap_or_default(),
      title: s.str("title").unwrap_or_default(),
      desc: s.str("desc").unwrap_or_default(),
      cover: s.str("cover").unwrap_or_default(),
      is_end: s.i64("isEnd").unwrap_or(0),
      price: s.i64("season_price").unwrap_or(0),
      paid: s.bool("is_pay").unwrap_or(false),
      activity: s.bool("isAct").unwrap_or(false),
      state,
      reject,
      sections: v
        .list("sections.sections")
        .iter()
        .filter_map(Section::parse)
        .collect(),
    })
  }

  fn count(&self) -> u64 {
    self.sections.iter().map(|s| s.count).sum()
  }

  fn url(&self) -> String {
    list_url(&self.mid, self.id)
  }

  /// Paid and activity seasons have flows of their own (prices, an
  /// organizer); ATP changes neither.
  fn writable(&self, what: &str) -> Result<()> {
    let kind = if self.paid {
      "a paid 合集 (付费合集)"
    } else if self.activity {
      "an activity 合集 (活动合集)"
    } else {
      return Ok(());
    };
    Err(Error::unsupported(format!("{what} of {kind}")))
  }

  /// `正片 (123), 番外 (456)`.
  fn section_names(&self) -> String {
    let names: Vec<String> = self
      .sections
      .iter()
      .map(|s| format!("{} ({})", s.title, s.id))
      .collect();
    names.join(", ")
  }
}

struct Section {
  id: u64,
  kind: i64,
  title: String,
  count: u64,
}

impl Section {
  fn parse(v: &Value) -> Option<Self> {
    Some(Self {
      id: v.u64("id")?,
      kind: v.i64("type").unwrap_or(1),
      title: v.str("title").unwrap_or_default(),
      count: v.count("epCount").unwrap_or(0),
    })
  }
}

/// A video in a season.
struct Episode {
  id: u64,
  aid: u64,
  bvid: String,
  title: String,
  section: u64,
  section_title: String,
  state: State,
  raw: Value,
}

impl Episode {
  fn parse(v: &Value, section: &Section) -> Option<Self> {
    let aid = v.u64("aid")?;
    let mut state = State::of(v);
    if state == State::Open && v.i64("archiveState") == Some(-50) {
      state = State::Private;
    }
    Some(Self {
      id: v.u64("id")?,
      aid,
      bvid: v.str("bvid").unwrap_or_else(|| Video::from_aid(aid).bvid),
      title: v.str("title").unwrap_or_default(),
      section: section.id,
      section_title: section.title.clone(),
      state,
      raw: v.clone(),
    })
  }

  /// The episode as a post; `position` counts from 1 through the season.
  fn post(&self, position: usize) -> Post {
    let mut p = Post {
      id: self.bvid.clone(),
      kind: "video".into(),
      title: Some(self.title.clone()),
      url: Some(format!("{VIDEO_URL}{}", self.bvid)),
      raw: Some(self.raw.clone()),
      ..Post::default()
    };
    let extra = [
      ("position", json!(position)),
      ("episode_id", json!(self.id)),
      ("section", json!(self.section_title)),
      ("section_id", json!(self.section)),
    ];
    p.extra
      .extend(extra.into_iter().map(|(k, v)| (k.to_owned(), v)));
    if self.state != State::Open {
      p.extra.insert("state".into(), json!(self.state.label()));
    }
    p
  }

  fn line(&self) -> String {
    format!("{} {}", self.bvid, self.title)
  }
}

/// A video on its way into a season.
struct NewEpisode {
  video: Video,
  title: String,
  cid: u64,
  charging: bool,
  member_first: bool,
  limited_free: bool,
}

impl NewEpisode {
  /// An item of `episodes`, as the page's 添加单集 sends it.
  fn body(&self) -> Value {
    json!({
      "title": self.title,
      "aid": self.video.aid,
      "cid": self.cid,
      "charging_pay": u8::from(self.charging),
      "member_first": u8::from(self.member_first),
      "limited_free": self.limited_free,
    })
  }
}

/// The new texts and cover of `season-create` and `season-edit`.
#[derive(Debug, Default)]
pub struct Info<'a> {
  pub title: Option<&'a str>,
  pub desc: Option<&'a str>,
  pub cover: Option<&'a Path>,
}

fn read<'a>(api: &'a Api, url: &str) -> Call<'a> {
  api.get(url).header("referer", PAGE)
}

fn write<'a>(api: &'a Api, url: &str) -> Call<'a> {
  api
    .post(url)
    .header("referer", PAGE)
    .header("origin", MEMBER)
}

/// Send a write. Bilibili answers 601 (with a voucher) when it wants a
/// captcha first: the page opens one and sends again with its token,
/// ATP does not. 20082 is the account's limit of 合集.
async fn send(call: Call<'_>) -> Result<Value> {
  call.send().await.map_err(|e| {
    if e.message.ends_with("(601)") {
      Error::verification(format!(
        "Bilibili wants a captcha for this 合集 change: {}",
        e.message
      ))
      .with_hint(format!("make the change on {PAGE}"))
      .unclearable()
    } else if e.message.ends_with("(20082)") {
      limit()
    } else {
      e
    }
  })
}

fn limit() -> Error {
  Error::new(
    ErrorCode::PermissionDenied,
    "the account has as many 合集 as Bilibili allows (当前合集数量达到上限)",
  )
  .with_hint("Bilibili's customer service can raise the limit")
}

/// The season on its owner's space, or the 合集 page when the owner is
/// not known.
fn list_url(mid: &str, id: u64) -> String {
  if mid.is_empty() || mid == "0" {
    return PAGE.to_owned();
  }
  format!("{}/lists/{id}?type=season", space_url(mid))
}

/// Your seasons in the page's order, with the drafts Bilibili suggests
/// (草稿) among them.
pub async fn list(api: &Api, page: &PageReq) -> Result<Page<Collection>> {
  api.require_login()?;
  let pn = Pn::of(page, 30);
  let data = read(api, SEASONS)
    .arg("pn", pn.pn)
    .arg("ps", pn.ps)
    .arg("order", "")
    .arg("sort", "")
    .arg("draft", 1)
    .arg("source", 0)
    .send()
    .await?;
  let items = data.list("seasons").iter().filter_map(collection).collect();
  Ok(pn.page(items, pn.before(data.u64("total"))))
}

fn collection(v: &Value) -> Option<Collection> {
  let s = Season::parse(v)?;
  let desc = one_line(&s.desc);
  let description = if s.state == State::Open {
    desc
  } else {
    format!("[{}] {desc}", s.state.name()).trim_end().to_owned()
  };
  let mut c = Collection {
    id: s.id.to_string(),
    kind: "season".into(),
    name: s.title.clone(),
    description: (!description.is_empty()).then_some(description),
    url: Some(s.url()),
    items: Some(s.count()),
    views: v.count("seasonStat.view"),
    raw: Some(v.clone()),
    ..Collection::default()
  };
  c.extra.insert("state".into(), json!(s.state.label()));
  c.extra.insert("sections".into(), json!(s.sections.len()));
  if let Some(reason) = &s.reject {
    c.extra.insert("reject_reason".into(), json!(reason));
  }
  Some(c)
}

/// One of your seasons with its sections.
async fn load(api: &Api, id: u64) -> Result<Season> {
  let data = read(api, SEASON).arg("id", id).send().await?;
  Season::parse(&data).ok_or_else(|| {
    Error::not_found(format!(
      "no 合集 {id} among yours (`atp bili seasons` lists them)"
    ))
  })
}

/// The episodes of every section, in order.
async fn episodes(api: &Api, season: &Season) -> Result<Vec<Episode>> {
  let mut out = Vec::new();
  for section in &season.sections {
    let data = read(api, SECTION).arg("id", section.id).send().await?;
    out.extend(
      data
        .list("episodes")
        .iter()
        .filter_map(|v| Episode::parse(v, section)),
    );
  }
  Ok(out)
}

/// The videos of one of your seasons, section by section in their order;
/// a note before them names the season, its state and its sections.
pub async fn show(api: &Api, id: u64) -> Result<Page<Post>> {
  api.require_login()?;
  let season = load(api, id).await?;
  let list = episodes(api, &season).await?;
  note(&outline(&season, &list));
  let posts = list
    .iter()
    .enumerate()
    .map(|(i, e)| e.post(i + 1))
    .collect();
  Ok(Page::new(posts, None))
}

/// `合集 123 操作系统: open (正常显示), 3 videos`, then the sections when
/// there are several, with the rows they take.
fn outline(s: &Season, list: &[Episode]) -> String {
  let mut lines = vec![format!(
    "合集 {} {}: {}, {} videos",
    s.id,
    s.title,
    s.state.label(),
    list.len()
  )];
  if let Some(reason) = &s.reject {
    lines.push(format!("  rejected: {reason}"));
  }
  if s.sections.len() > 1 {
    let mut start = 1;
    for section in &s.sections {
      let n = list.iter().filter(|e| e.section == section.id).count();
      let rows = match n {
        0 => "empty".to_owned(),
        1 => format!("#{start}"),
        _ => format!("#{start}–#{}", start + n - 1),
      };
      lines.push(format!(
        "  section {} ({}): {rows}",
        section.title, section.id
      ));
      start += n;
    }
  }
  lines.join("\n")
}

fn check_texts(title: &str, desc: &str) -> Result<()> {
  if title.is_empty() {
    return Err(Error::input("a 合集 needs a title"));
  }
  if title.chars().count() > TITLE_MAX {
    return Err(Error::input(format!(
      "a 合集's title takes at most {TITLE_MAX} characters"
    )));
  }
  if desc.chars().count() > DESC_MAX {
    return Err(Error::input(format!(
      "a 合集's description takes at most {DESC_MAX} characters"
    )));
  }
  Ok(())
}

fn shown(text: &str) -> String {
  let line = one_line(text);
  if line.is_empty() {
    "(none)".to_owned()
  } else {
    truncate(&line, 60)
  }
}

/// A note on a cover more than 15% off 16:9: the page crops a picked
/// image to it, ATP sends it as it is.
async fn cover_note(path: &Path) -> Option<String> {
  let (width, height) = Image::read(path).await.ok()?.size();
  if width == 0 || height == 0 {
    return None;
  }
  let off = (width as f64 / height as f64) / (16.0 / 9.0);
  (!(0.85..=1.15).contains(&off)).then(|| {
    format!(
      "{} is {width}×{height}, far from 16:9; it goes uncropped (the web page crops it to 16:9)",
      path.display()
    )
  })
}

/// Create a season, after showing it and asking; its id comes back.
pub async fn create(api: &Api, info: &Info<'_>, yes: bool) -> Result<Action> {
  api.require_login()?;
  let title = info.title.unwrap_or_default().trim();
  let desc = info.desc.unwrap_or_default().trim();
  check_texts(title, desc)?;
  if let Some(path) = info.cover {
    atp_core::cli::check_images(&[path.to_path_buf()])?;
  }
  // The page asks how many more the account may make before it opens
  // the form; a failed question is left to the create itself.
  if let Ok(left) = read(api, REMAIN).send().await
    && left.i64("").is_some_and(|n| n <= 0)
  {
    return Err(limit());
  }
  let mut change = Change::new("New Bilibili 合集:");
  change.row("title", title).row("desc", &shown(desc));
  match info.cover {
    Some(path) => {
      change.row("cover", &path.display().to_string());
      if let Some(text) = cover_note(path).await {
        change.row("", &text);
      }
    }
    None => {
      change.row("cover", "the upload page's default picture (no --cover)");
    }
  }
  change.row("review", "Bilibili reviews the 合集 before it shows");
  change.confirm(yes, "create this 合集 on Bilibili")?;
  let cover = match info.cover {
    Some(path) => uploads::cover(api, path, "cover", PAGE).await?,
    None => DEFAULT_COVER.to_owned(),
  };
  let data = send(write(api, ADD).json(create_body(title, desc, &cover))).await?;
  // `data` is the new id (the page selects the season by it).
  let id = data
    .u64("")
    .or_else(|| data.u64("id"))
    .ok_or_else(|| Error::upstream("Bilibili named no id for the new 合集"))?;
  let mid = users::my_mid(api).await.unwrap_or_default();
  Ok(
    Action::done("season-create", title)
      .with_id(id.to_string())
      .with_url(list_url(&mid, id))
      .with_message(format!(
        "Bilibili reviews it; add videos with `atp bili season-add {id} BV…`"
      )),
  )
}

/// The page's `addEp`: the form, the price of a free season, and the
/// empty captcha token it sends first.
fn create_body(title: &str, desc: &str, cover: &str) -> Value {
  json!({
    "cover": cover,
    "title": title,
    "desc": desc,
    "season_price": 0,
    "captcha_token": "",
  })
}

/// Change a season's title, description or cover; the page's edit sends
/// the whole season with its sections' order, so the rest goes back as
/// it was.
pub async fn edit(api: &Api, id: u64, info: &Info<'_>, yes: bool) -> Result<Action> {
  api.require_login()?;
  if info.title.is_none() && info.desc.is_none() && info.cover.is_none() {
    return Err(Error::input(
      "nothing to change: give --title, --desc or --cover",
    ));
  }
  if let Some(path) = info.cover {
    atp_core::cli::check_images(&[path.to_path_buf()])?;
  }
  let s = load(api, id).await?;
  s.writable("season-edit")?;
  let title = info.title.map_or(s.title.as_str(), str::trim);
  let desc = info.desc.map_or(s.desc.as_str(), str::trim);
  check_texts(title, desc)?;
  let mut change = Change::new(format!("Bilibili 合集 {id}:"));
  change
    .field("title", &s.title, title)
    .field("desc", &s.desc, desc);
  if let Some(path) = info.cover {
    change.row("cover", &path.display().to_string());
    if let Some(text) = cover_note(path).await {
      change.row("", &text);
    }
  }
  change.row("review", "Bilibili reviews the change");
  change.confirm(yes, "change this 合集 on Bilibili")?;
  let cover = match info.cover {
    Some(path) => uploads::cover(api, path, "cover", PAGE).await?,
    None => uploads::https(&s.cover),
  };
  send(write(api, EDIT).json(edit_body(&s, title, desc, &cover))).await?;
  Ok(
    Action::done("season-edit", id.to_string())
      .with_url(s.url())
      .with_message("Bilibili reviews the change"),
  )
}

/// The page's `editEp`: the season with the fields it shows, the order of
/// its sections, and the empty captcha token inside `season`.
fn edit_body(s: &Season, title: &str, desc: &str, cover: &str) -> Value {
  let sorts: Vec<Value> = s
    .sections
    .iter()
    .enumerate()
    .map(|(i, x)| json!({ "id": x.id, "sort": i + 1 }))
    .collect();
  json!({
    "season": {
      "id": s.id,
      "title": title,
      "desc": desc,
      "cover": cover,
      "isEnd": s.is_end,
      "season_price": s.price,
      "captcha_token": "",
    },
    "sorts": sorts,
  })
}

/// Delete a season after asking; its videos stay published.
pub async fn delete(api: &Api, id: u64, yes: bool) -> Result<Action> {
  api.require_login()?;
  let s = load(api, id).await?;
  let mut change = Change::new(format!("Bilibili 合集 {id}:"));
  change
    .row("title", &s.title)
    .row("state", &s.state.label())
    .row(
      "videos",
      &format!("{}; they stay published, in no 合集", s.count()),
    );
  change.confirm(yes, "delete this 合集 on Bilibili")?;
  send(write(api, DEL).arg("id", id)).await?;
  Ok(
    Action::done("season-delete", id.to_string())
      .with_message(format!("deleted 合集 {}; its videos stay", s.title)),
  )
}

/// Each video once, in the order given.
fn unique(videos: &[Video]) -> Vec<&Video> {
  let mut out: Vec<&Video> = Vec::new();
  for v in videos {
    if !out.iter().any(|o| o.aid == v.aid) {
      out.push(v);
    }
  }
  out
}

/// Errors about one video (not a login, a check or the network, which
/// every video would meet).
fn about_video(e: &Error) -> bool {
  matches!(
    e.code,
    ErrorCode::NotFound
      | ErrorCode::PermissionDenied
      | ErrorCode::InvalidInput
      | ErrorCode::UpstreamError
  )
}

/// A video as the page takes it in: Bilibili's check first
/// (`verifyAidCanAdd`), then its record for the cid of its first part
/// and its title.
async fn candidate(api: &Api, v: &Video, mine: &str) -> Result<NewEpisode> {
  read(api, VERIFY)
    .arg("aid", v.aid)
    .arg("from", 0)
    .send()
    .await
    .map_err(|e| {
      if about_video(&e) {
        Error::new(
          ErrorCode::PermissionDenied,
          format!("Bilibili does not let it in: {}", e.message),
        )
      } else {
        e
      }
    })?;
  let data = read(api, ARCHIVE)
    .arg("aid", v.aid)
    .send()
    .await
    .map_err(|e| {
      if about_video(&e) {
        Error::not_found(format!("not among your videos ({})", e.message))
      } else {
        e
      }
    })?;
  let a = data.at("archive");
  if let Some(mid) = a.str("mid")
    && mid != mine
  {
    return Err(Error::new(
      ErrorCode::PermissionDenied,
      "not your video: a 合集 takes your own videos only",
    ));
  }
  let part = data.list("videos").first();
  let cid = part
    .and_then(|p| p.u64("cid"))
    .ok_or_else(|| Error::not_found("Bilibili lists no part for it (查询分p异常)"))?;
  let title = a
    .str("title")
    .or_else(|| part.and_then(|p| p.str("title")))
    .unwrap_or_else(|| v.bvid.clone());
  let flag = |key: &str| a.bool(key).unwrap_or(false);
  Ok(NewEpisode {
    video: v.clone(),
    title,
    cid,
    charging: flag("charging_pay"),
    member_first: flag("charge_2_free"),
    limited_free: flag("limited_free"),
  })
}

/// The section `wanted`, or the last one, which the upload page picks.
fn pick_section(s: &Season, wanted: Option<u64>) -> Result<&Section> {
  match wanted {
    Some(id) => s.sections.iter().find(|x| x.id == id).ok_or_else(|| {
      Error::input(format!(
        "合集 {} has no section {id}; its sections: {}",
        s.id,
        s.section_names()
      ))
    }),
    None => s.sections.last().ok_or_else(|| {
      Error::upstream(format!(
        "合集 {} has no section to add to yet; look at it on {PAGE}",
        s.id
      ))
    }),
  }
}

/// Add videos to a season, in the order given, at the end of a section.
/// Every video is checked first; one that cannot go in stops the command
/// before anything changes.
pub async fn add(
  api: &Api,
  id: u64,
  videos: &[Video],
  section: Option<u64>,
  yes: bool,
) -> Result<Vec<Action>> {
  api.require_login()?;
  let s = load(api, id).await?;
  s.writable("season-add")?;
  let target = pick_section(&s, section)?;
  let existing = episodes(api, &s).await?;
  let mine = users::my_mid(api).await?;
  let mut picked: Vec<NewEpisode> = Vec::new();
  let mut refused: Vec<String> = Vec::new();
  for v in unique(videos) {
    if let Some(at) = existing.iter().position(|e| e.aid == v.aid) {
      refused.push(format!("{}: already in this 合集 (#{})", v.bvid, at + 1));
      continue;
    }
    match candidate(api, v, &mine).await {
      Ok(ep) => picked.push(ep),
      Err(e) if about_video(&e) => refused.push(format!("{}: {}", v.bvid, e.message)),
      Err(e) => return Err(e),
    }
  }
  if !refused.is_empty() {
    return Err(
      Error::new(
        ErrorCode::PermissionDenied,
        format!(
          "these videos cannot go into 合集 {id}:\n  {}",
          refused.join("\n  ")
        ),
      )
      .with_hint(
        "a 合集 takes your own videos, each in one 合集 only, single-part unless Bilibili allows more; \
         `atp bili season-remove OTHER BV…` takes one out of another 合集",
      ),
    );
  }
  let before = existing.iter().filter(|e| e.section == target.id).count();
  let mut change = Change::new(format!("Bilibili 合集 {id} {}:", s.title));
  change.row(
    "section",
    &format!(
      "{} ({}), after its {before} videos",
      target.title, target.id
    ),
  );
  for (i, e) in picked.iter().enumerate() {
    change.row(
      &format!("#{}", before + i + 1),
      &format!("{} {}", e.video.bvid, e.title),
    );
  }
  change.confirm(yes, "add these videos to the 合集")?;
  let chunks: Vec<&[NewEpisode]> = picked.chunks(BATCH).collect();
  let mut done: Vec<Action> = Vec::with_capacity(picked.len());
  for (i, chunk) in chunks.iter().enumerate() {
    let body = add_body(target.id, chunk);
    match send(write(api, EPISODES_ADD).json(body)).await {
      Ok(_) => done.extend(chunk.iter().map(|e| {
        Action::done("season-add", &e.video.bvid)
          .with_id(&e.video.bvid)
          .with_url(e.video.url())
      })),
      Err(e) if done.is_empty() => return Err(e),
      // The order matters: what follows a failed batch is not sent.
      Err(e) => {
        let failed = chunk
          .iter()
          .map(|x| Action::failed("season-add", &x.video.bvid, &e));
        let skipped = chunks[i + 1..]
          .iter()
          .flat_map(|c| c.iter())
          .map(|x| bulk::skipped("season-add", &x.video.bvid, &e));
        done.extend(failed.chain(skipped));
        break;
      }
    }
  }
  Ok(done)
}

/// The page's 添加单集: the section and its new episodes.
fn add_body(section: u64, episodes: &[NewEpisode]) -> Value {
  let items: Vec<Value> = episodes.iter().map(NewEpisode::body).collect();
  json!({ "sectionId": section, "episodes": items })
}

/// The season's episodes of `videos`, in the order given; a video not in
/// the season stops the command.
fn named<'a>(id: u64, list: &'a [Episode], videos: &[Video]) -> Result<Vec<&'a Episode>> {
  let mut found = Vec::new();
  let mut missing = Vec::new();
  for v in unique(videos) {
    match list.iter().find(|e| e.aid == v.aid) {
      Some(e) => found.push(e),
      None => missing.push(v.bvid.clone()),
    }
  }
  if missing.is_empty() {
    Ok(found)
  } else {
    Err(Error::not_found(format!(
      "not in 合集 {id}: {} (`atp bili season {id}` lists it)",
      missing.join(", ")
    )))
  }
}

/// Take videos out of a season, one call each, after asking; they stay
/// published.
pub async fn remove(api: &Api, id: u64, videos: &[Video], yes: bool) -> Result<Vec<Action>> {
  api.require_login()?;
  let s = load(api, id).await?;
  s.writable("season-remove")?;
  let list = episodes(api, &s).await?;
  let picked = named(id, &list, videos)?;
  let mut change = Change::new(format!("Bilibili 合集 {id} {}, take out:", s.title));
  for e in &picked {
    let at = list.iter().position(|x| x.id == e.id).unwrap_or_default();
    change.row(&format!("#{}", at + 1), &e.line());
  }
  change.row("", "the videos stay published");
  change.confirm(yes, "take these videos out of the 合集")?;
  let names: Vec<String> = picked.iter().map(|e| e.bvid.clone()).collect();
  bulk::each("season-remove", &names, async |bvid| {
    let e = picked
      .iter()
      .find(|e| e.bvid == bvid)
      .ok_or_else(|| Error::internal(format!("{bvid} is not among the picked videos")))?;
    send(write(api, EPISODE_DEL).arg("id", e.id)).await?;
    Ok(Action::done("season-remove", bvid).with_id(bvid))
  })
  .await
}

/// Put the videos of one section in the order given: those named first,
/// the others after them as they were. The section's whole list goes out
/// (`section/edit`), after asking.
pub async fn sort(api: &Api, id: u64, videos: &[Video], yes: bool) -> Result<Action> {
  api.require_login()?;
  let s = load(api, id).await?;
  s.writable("season-sort")?;
  let list = episodes(api, &s).await?;
  let picked = named(id, &list, videos)?;
  let section_id = picked.first().map(|e| e.section).unwrap_or_default();
  if picked.iter().any(|e| e.section != section_id) {
    return Err(Error::input(format!(
      "the videos are in different sections of 合集 {id} ({}); sort one section at a time",
      s.section_names()
    )));
  }
  let section = s
    .sections
    .iter()
    .find(|x| x.id == section_id)
    .ok_or_else(|| Error::upstream(format!("合集 {id} lists no section {section_id}")))?;
  let current: Vec<&Episode> = list.iter().filter(|e| e.section == section_id).collect();
  let order = reorder(&picked, &current);
  if order.iter().map(|e| e.id).eq(current.iter().map(|e| e.id)) {
    return Ok(
      Action::done("season-sort", id.to_string())
        .with_message("the videos are in this order already"),
    );
  }
  let mut change = Change::new(format!(
    "Bilibili 合集 {id} {}, section {} ({}), new order:",
    s.title, section.title, section.id
  ));
  for (i, e) in order.iter().enumerate() {
    let was = current
      .iter()
      .position(|c| c.id == e.id)
      .unwrap_or_default()
      + 1;
    let moved = if was == i + 1 {
      String::new()
    } else {
      format!(" (was #{was})")
    };
    change.row(&format!("#{}", i + 1), &format!("{}{moved}", e.line()));
  }
  change.confirm(yes, "reorder the 合集 on Bilibili")?;
  send(write(api, SECTION_EDIT).json(sort_body(id, section, &order))).await?;
  Ok(Action::done("season-sort", id.to_string()).with_url(s.url()))
}

/// `picked` first, then the rest of `current` as it was.
fn reorder<'a>(picked: &[&'a Episode], current: &[&'a Episode]) -> Vec<&'a Episode> {
  let mut order = picked.to_vec();
  order.extend(
    current
      .iter()
      .filter(|e| !picked.iter().any(|p| p.id == e.id)),
  );
  order
}

/// The page's `editSection`: the section as it is, every episode with its
/// new place, and the empty captcha token at the top.
fn sort_body(season: u64, section: &Section, order: &[&Episode]) -> Value {
  let sorts: Vec<Value> = order
    .iter()
    .enumerate()
    .map(|(i, e)| json!({ "id": e.id, "sort": i + 1 }))
    .collect();
  json!({
    "section": {
      "id": section.id,
      "type": section.kind,
      "seasonId": season,
      "title": section.title,
    },
    "sorts": sorts,
    "captcha_token": "",
  })
}
