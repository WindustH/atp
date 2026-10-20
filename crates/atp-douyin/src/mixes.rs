//! Your collections (合集) as the creator platform's 合集管理 pages
//! (`content/collection/…`) keep them: creating one with its title,
//! description, cover and first works (`mix/create/`), changing those
//! (`mix/update/`), deleting it (`mix/destroy/`), and the works in it,
//! added, taken out and put in order through one call that carries the
//! whole new list (`mix/item/editlist/`).
//!
//! These are the 合集 pages' own calls; a work's edit page, whose
//! `update/desc/` counts the 5 edits a work has, hides the publish form's
//! 合集 field. So none of them uses an edit. Nothing goes out before the
//! person has seen the change and said yes.

use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;

use atp_core::cli::Change;
use atp_core::output::note;
use atp_core::{Action, Error, ErrorCode, Result, Value, ValueExt};

use crate::api::{Api, WWW};
use crate::creator::Creator;
use crate::items::Form;
use crate::uploads::Uploader;
use crate::{refs, users};

const LIST: &str = "/web/api/mix/list/";
const DETAIL: &str = "/web/api/mix/detail/";
const ITEMS: &str = "/web/api/mix/item/list/";
const CREATE: &str = "/web/api/mix/create/";
const UPDATE: &str = "/web/api/mix/update/";
const EDIT_LIST: &str = "/web/api/mix/item/editlist/";
const DESTROY: &str = "/web/api/mix/destroy/";
const ITEM_INFO: &str = "/web/api/media/item/info/";
/// The 合集管理 pages, the referers of their calls.
const PAGES: &str = "https://creator.douyin.com/creator-micro/content/collection";

/// The manage page's 全部合集 tab: 待发布, 审核中, 已发布, 未通过 (仅自己可见)
/// and live replays.
const ALL: &str = "0,1,2,3,6";
const LIST_PAGES: usize = 20;
/// Pages of 50 works the detail page loads, at most here.
const ITEM_PAGES: usize = 100;

/// The page's limits, in JavaScript string lengths.
const TITLE: RangeInclusive<usize> = 2..=20;
const DESC: RangeInclusive<usize> = 5..=200;
/// Works one change adds at most (单次添加进合集的视频不可以超过100个).
const ADD_MAX: usize = 100;
/// The cover picker: JPG or PNG, 5 MB, 100×100 at least.
const COVER_BYTES: u64 = 5 * 1024 * 1024;
const COVER_MIN: usize = 100;
/// The cover the page counts as none.
const NO_COVER: &str = "tos-cn-i-dy/2e399e3e9f2941c680b3023c48cfdc9f";
/// Works a collection needs for others to see it.
const SHOWN_MIN: usize = 2;

/// `status.status` of a collection.
const REVIEWING: i64 = 1;
const MADE_BY_DOUYIN: i64 = 5;
/// `type` of a collection: a plain one, live replays (直播回放), a short
/// drama (短剧) and one for members (会员合集).
const NORMAL: i64 = 0;
const LIVE_REPLAYS: i64 = 2;
const PLAYLET: i64 = 10;
const MEMBERS: i64 = 13;

const EDITS: &str = "uses none of the works' 5 edits (the 合集 page's own call, not the edit page)";
const KEPT: &str = "the works stay on your account (移出合集后将不会删除原视频)";

/// The title, description and cover of `mix-create` and `mix-edit`.
#[derive(Debug, Default)]
pub struct MixSpec {
  pub title: Option<String>,
  pub desc: Option<String>,
  pub cover: Option<PathBuf>,
}

/// A collection as the detail page loads it (`mix/detail/`, a change
/// still in review, `next_info`, over it) and, when asked for, its works
/// in episode order (`mix/item/list/`).
struct Mix {
  id: String,
  name: String,
  desc: String,
  cover: String,
  status: i64,
  kind: i64,
  paid: bool,
  /// The episodes Douyin counts (`statis.updated_to_episode`).
  episodes: u64,
  works: Vec<Entry>,
}

/// A work as a list shows it.
#[derive(Debug)]
struct Entry {
  id: String,
  label: String,
  /// Its cover, as the page picks one for a collection without its own.
  cover: String,
}

/// Create a collection with `works` as its first episodes, in order.
pub async fn create(api: &Api, spec: &MixSpec, works: &[String], yes: bool) -> Result<Action> {
  let title = checked_title(spec.title.as_deref().unwrap_or_default())?;
  let desc = checked_desc(spec.desc.as_deref())?.unwrap_or_default();
  let ids = work_ids(api, works).await?;
  if ids.len() > ADD_MAX {
    return Err(too_many());
  }
  let cover_notes = match &spec.cover {
    Some(path) => check_cover(path).await?,
    None => Vec::new(),
  };
  let creator = Creator::open(api).await?;
  let mut entries = Vec::with_capacity(ids.len());
  for id in &ids {
    entries.push(work(&creator, id, None).await?);
  }
  let mut change = Change::new("Douyin collection (合集), new:");
  change.row("title", &title);
  change.row(
    "description",
    if desc.is_empty() { "(none)" } else { &desc },
  );
  match &spec.cover {
    Some(path) => change.row("cover", &file_row(path)),
    None if entries.is_empty() => change.row("cover", "(none)"),
    None => change.row("cover", "the last work's cover, as the page picks it"),
  };
  for text in &cover_notes {
    change.row("", text);
  }
  change.row("works", &entries.len().to_string());
  for (i, e) in entries.iter().enumerate() {
    change.row(&format!("episode {}", i + 1), &e.row());
  }
  if !entries.is_empty() {
    change.row("edits", EDITS);
  }
  for text in count_notes(entries.len()) {
    change.row("", text);
  }
  change.row(
    "",
    "Douyin reviews the title, cover and description; while the collection is in \
     review (审核中) nothing in it can change",
  );
  change.confirm(yes, "create this collection on Douyin")?;

  let cover = match &spec.cover {
    Some(path) => upload_cover(api, &creator, path).await?,
    None => entries.last().map(|e| e.cover.clone()).unwrap_or_default(),
  };
  let form = create_form(&title, &desc, &cover, &ids);
  pause(api).await;
  let v = creator
    .post_form(CREATE, &form)
    .referer(&format!("{PAGES}/create"))
    .send()
    .await?;
  let id = v.first_str(&[
    "mix_id_str",
    "mix_id",
    "mix_info.mix_id_str",
    "mix_info.mix_id",
  ]);
  let action = Action::done("mix-create", id.as_deref().unwrap_or(&title));
  Ok(match id {
    Some(id) => action.with_url(url(&id)).with_id(id),
    None => {
      action.with_message("created; the mix-* commands find it by its title (Douyin named no id)")
    }
  })
}

/// Change a collection's title, description or cover.
pub async fn edit(api: &Api, arg: &str, spec: &MixSpec, yes: bool) -> Result<Action> {
  if spec.title.is_none() && spec.desc.is_none() && spec.cover.is_none() {
    return Err(Error::input(
      "nothing to change: give --title, --desc or --cover",
    ));
  }
  let title = spec.title.as_deref().map(checked_title).transpose()?;
  let desc = checked_desc(spec.desc.as_deref())?;
  let cover_notes = match &spec.cover {
    Some(path) => check_cover(path).await?,
    None => Vec::new(),
  };
  let creator = Creator::open(api).await?;
  let mix = Mix::find(&creator, arg, true).await?;
  mix.check_changeable()?;
  let title = title.unwrap_or_else(|| mix.name.clone());
  let desc = desc.unwrap_or_else(|| mix.desc.clone());
  if title == mix.name && desc == mix.desc && spec.cover.is_none() {
    return Err(Error::input(
      "nothing changes: the collection has these already",
    ));
  }
  let mut change = mix.change();
  change
    .field("title", &mix.name, &title)
    .field("description", &mix.desc, &desc);
  if let Some(path) = &spec.cover {
    change.row("cover", &file_row(path));
  }
  for text in &cover_notes {
    change.row("", text);
  }
  change.row("", "Douyin reviews the change before it shows");
  change.confirm(yes, "change this collection on Douyin")?;

  let cover = match &spec.cover {
    Some(path) => upload_cover(api, &creator, path).await?,
    None => mix.kept_cover(),
  };
  let form = update_form(&mix, &title, &desc, &cover);
  pause(api).await;
  creator
    .post_form(UPDATE, &form)
    .referer(&mix.page())
    .send()
    .await?;
  Ok(mix.done("mix-edit"))
}

/// Delete a collection; its works stay.
pub async fn delete(api: &Api, arg: &str, yes: bool) -> Result<Action> {
  let creator = Creator::open(api).await?;
  let mix = Mix::find(&creator, arg, false).await?;
  if mix.kind == LIVE_REPLAYS {
    return Err(Error::unsupported(
      "deleting a collection of live replays (直播回放), which the page does not offer",
    ));
  }
  if mix.paid && mix.episodes > 0 {
    return Err(Error::new(
      ErrorCode::PermissionDenied,
      "Douyin: a paid collection with published works cannot be deleted (已发布作品的付费合集暂不允许删除)",
    ));
  }
  let mut change = mix.change();
  change.row("works", &mix.episodes.to_string());
  change.row(
    "",
    "the works stay on your account (删除合集时不会删除合集里的视频)",
  );
  change.confirm(yes, "delete this collection on Douyin")?;
  pause(api).await;
  creator
    .post_form(DESTROY, &[("mix_id".into(), mix.id.clone())])
    .referer(&format!("{PAGES}/manage"))
    .send()
    .await?;
  Ok(mix.done("mix-delete"))
}

/// Add works to a collection as its next episodes, in the order given.
pub async fn add(api: &Api, arg: &str, works: &[String], yes: bool) -> Result<Action> {
  let ids = work_ids(api, works).await?;
  if ids.len() > ADD_MAX {
    return Err(too_many());
  }
  let creator = Creator::open(api).await?;
  let mix = Mix::find(&creator, arg, true).await?;
  mix.check_changeable()?;
  if let Some(id) = ids.iter().find(|id| mix.position(id).is_some()) {
    return Err(Error::input(format!(
      "work {id} is in this collection already (已在当前合集); `mix-sort` moves it"
    )));
  }
  let mut added = Vec::with_capacity(ids.len());
  for id in &ids {
    added.push(work(&creator, id, Some(&mix.id)).await?);
  }
  let list: Vec<&Entry> = mix.works.iter().chain(&added).collect();
  let mut change = mix.change();
  change.field(
    "works",
    &mix.works.len().to_string(),
    &list.len().to_string(),
  );
  for (i, e) in added.iter().enumerate() {
    let label = if i == 0 { "add" } else { "" };
    let episode = mix.works.len() + i + 1;
    change.row(label, &format!("episode {episode}: {}", e.row()));
  }
  change.row("edits", EDITS);
  for text in count_notes(list.len()) {
    change.row("", text);
  }
  change.confirm(yes, "add these works to the collection on Douyin")?;
  mix.send_list(api, &creator, &list).await?;
  Ok(mix.done("mix-add"))
}

/// Take works out of a collection; they stay on the account.
pub async fn remove(api: &Api, arg: &str, works: &[String], yes: bool) -> Result<Action> {
  let ids = work_ids(api, works).await?;
  let creator = Creator::open(api).await?;
  let mix = Mix::find(&creator, arg, true).await?;
  mix.check_changeable()?;
  let mut change = mix.change();
  for (i, id) in ids.iter().enumerate() {
    let at = mix.position(id).ok_or_else(|| not_in(id))?;
    let label = if i == 0 { "remove" } else { "" };
    let e = &mix.works[at];
    change.row(label, &format!("episode {}: {}", at + 1, e.row()));
  }
  let list: Vec<&Entry> = mix.works.iter().filter(|e| !ids.contains(&e.id)).collect();
  change.field(
    "works",
    &mix.works.len().to_string(),
    &list.len().to_string(),
  );
  change.row("", KEPT);
  change.row("edits", EDITS);
  for text in count_notes(list.len()) {
    change.row("", text);
  }
  change.confirm(yes, "take these works out of the collection on Douyin")?;
  mix.send_list(api, &creator, &list).await?;
  Ok(mix.done("mix-remove"))
}

/// Put a collection's works in a new order: the works given first, in
/// that order, then the others as they were.
pub async fn sort(api: &Api, arg: &str, works: &[String], yes: bool) -> Result<Action> {
  let ids = work_ids(api, works).await?;
  let creator = Creator::open(api).await?;
  let mix = Mix::find(&creator, arg, true).await?;
  mix.check_changeable()?;
  let mut list = Vec::with_capacity(mix.works.len());
  for id in &ids {
    let at = mix.position(id).ok_or_else(|| not_in(id))?;
    list.push(&mix.works[at]);
  }
  list.extend(mix.works.iter().filter(|e| !ids.contains(&e.id)));
  let moved: Vec<(usize, usize, &Entry)> = list
    .iter()
    .enumerate()
    .filter_map(|(new, e)| {
      let old = mix.position(&e.id)?;
      (old != new).then_some((old, new, *e))
    })
    .collect();
  if moved.is_empty() {
    return Err(Error::input(
      "nothing changes: the works are in this order already",
    ));
  }
  let mut change = mix.change();
  for (i, (old, new, e)) in moved.iter().enumerate() {
    let label = if i == 0 { "order" } else { "" };
    change.row(
      label,
      &format!("episode {}: {} (was {})", new + 1, e.row(), old + 1),
    );
  }
  change.row("edits", EDITS);
  change.confirm(yes, "put the collection's works in this order on Douyin")?;
  mix.send_list(api, &creator, &list).await?;
  Ok(mix.done("mix-sort"))
}

impl Mix {
  /// One of the account's collections by id, link or exact title, from
  /// the manage page's list; with `works`, its works too.
  async fn find(creator: &Creator<'_>, arg: &str, works: bool) -> Result<Self> {
    let id = find_id(creator, arg).await?;
    let mut mix = Self::load(creator, &id).await?;
    if works {
      mix.works = list_works(creator, &mix).await?;
    }
    Ok(mix)
  }

  /// `mix/detail/`, as the detail page asks for it; a change in review
  /// (`next_info`) shows over the collection, as on the page, unless
  /// Douyin made the collection.
  async fn load(creator: &Creator<'_>, id: &str) -> Result<Self> {
    let v = creator
      .get(DETAIL)
      .arg("mix_id", id)
      .arg("need_charge_skus", true)
      .referer(&format!("{PAGES}/detail/{id}"))
      .send()
      .await?;
    let info = v.at("mix_info");
    if info.is_null() {
      return Err(not_found(id));
    }
    let status = info.i64("status.status").unwrap_or(0);
    let next = match status {
      MADE_BY_DOUYIN => &Value::Null,
      _ => info.at("next_info"),
    };
    let pick = |path: &str| {
      next
        .str(path)
        .or_else(|| info.str(path))
        .unwrap_or_default()
    };
    Ok(Self {
      id: id.to_owned(),
      name: pick("mix_name"),
      desc: pick("desc"),
      cover: pick("cover_url.uri"),
      status,
      kind: info
        .first_str(&["type", "mix_type"])
        .and_then(|t| t.parse().ok())
        .unwrap_or(NORMAL),
      paid: info.i64("is_charge_series") == Some(1),
      episodes: info.u64("statis.updated_to_episode").unwrap_or(0),
      works: Vec::new(),
    })
  }

  /// Fails where the page lets no change through: a collection in
  /// review, and the kinds it handles with other forms.
  fn check_changeable(&self) -> Result<()> {
    if self.status == REVIEWING {
      return Err(Error::new(
        ErrorCode::PermissionDenied,
        "Douyin: the collection is in review (合集审核中，不允许修改); change it once the review is over",
      ));
    }
    if self.paid {
      return Err(Error::unsupported(
        "changing a paid collection (付费合集), whose works the page adds when they are published",
      ));
    }
    let kind = match self.kind {
      LIVE_REPLAYS => "live replays (直播回放)",
      PLAYLET => "a short drama (短剧)",
      MEMBERS => "for members (会员合集)",
      _ => return Ok(()),
    };
    Err(Error::unsupported(format!(
      "changing a collection of {kind}, which the page handles with other forms"
    )))
  }

  fn position(&self, id: &str) -> Option<usize> {
    self.works.iter().position(|e| e.id == id)
  }

  /// The summary's head: the collection, its state and its works.
  fn change(&self) -> Change {
    let mut change = Change::new(format!("Douyin collection {} {}:", self.id, self.name));
    change.row("status", status_name(self.status));
    if !self.works.is_empty() && self.works.len() as u64 != self.episodes {
      change.row(
        "",
        &format!(
          "Douyin counts {} episodes, its list holds {} works",
          self.episodes,
          self.works.len()
        ),
      );
    }
    change
  }

  /// The cover `mix/update/` keeps: the collection's own, or (when it has
  /// none) the last work's, as the page fills it in.
  fn kept_cover(&self) -> String {
    if !self.cover.is_empty() && self.cover != NO_COVER {
      return self.cover.clone();
    }
    match self.works.last() {
      Some(e) if !e.cover.is_empty() => e.cover.clone(),
      _ => self.cover.clone(),
    }
  }

  /// Send the works in their new order (`mix/item/editlist/`).
  async fn send_list(&self, api: &Api, creator: &Creator<'_>, list: &[&Entry]) -> Result<()> {
    let form = edit_list_form(&self.id, &self.works, list);
    pause(api).await;
    creator
      .post_form(EDIT_LIST, &form)
      .referer(&self.page())
      .send()
      .await?;
    Ok(())
  }

  fn page(&self) -> String {
    format!("{PAGES}/detail/{}", self.id)
  }

  fn done(&self, action: &str) -> Action {
    Action::done(action, &self.id)
      .with_id(&self.id)
      .with_url(url(&self.id))
  }
}

impl Entry {
  /// A work of the lists Douyin gives: its id, title (or text) and cover.
  fn of(aweme: &Value) -> Option<Self> {
    let id = aweme.first_str(&["aweme_id", "item_id"])?;
    Some(Self::with_id(id, aweme))
  }

  fn with_id(id: String, aweme: &Value) -> Self {
    let text = aweme
      .first_str(&["item_title", "desc"])
      .unwrap_or_else(|| "(no text)".into());
    Self {
      id,
      label: atp_core::text::truncate(&atp_core::text::one_line(&text), 30),
      cover: aweme_cover(aweme),
    }
  }

  fn row(&self) -> String {
    format!("{} {}", self.id, self.label)
  }
}

/// The id of one of the account's collections: `arg` is an id, a
/// douyin.com/collection/… link or the exact title (also one still in
/// review), as the manage page lists them.
async fn find_id(creator: &Creator<'_>, arg: &str) -> Result<String> {
  let arg = arg.trim();
  let wanted = refs::mix(arg).ok();
  let mut cursor = 0;
  for _ in 0..LIST_PAGES {
    let v = creator
      .get(LIST)
      .arg("status", ALL)
      .arg("count", 20)
      .arg("cursor", cursor)
      .referer(&format!("{PAGES}/manage"))
      .send()
      .await?;
    let found = v.list("mix_list").iter().find_map(|m| {
      let id = m.first_str(&["mix_id_str", "mix_id"])?;
      let named = [m.str("mix_name"), m.str("next_info.mix_name")]
        .into_iter()
        .flatten()
        .any(|name| name.trim() == arg);
      (wanted.as_deref() == Some(id.as_str()) || named).then_some(id)
    });
    if let Some(id) = found {
      return Ok(id);
    }
    match v.u64("cursor") {
      Some(next) if v.bool("has_more") == Some(true) && next != cursor => cursor = next,
      _ => break,
    }
  }
  Err(not_found(arg))
}

/// The works of a collection in episode order, every page of them (the
/// new list replaces the whole one, so a part will not do).
async fn list_works(creator: &Creator<'_>, mix: &Mix) -> Result<Vec<Entry>> {
  let mut works: Vec<Entry> = Vec::new();
  let mut cursor = 0;
  for _ in 0..ITEM_PAGES {
    let v = creator
      .get(ITEMS)
      .arg("mix_id", &mix.id)
      .arg("count", 50)
      .arg("cursor", cursor)
      .referer(&mix.page())
      .send()
      .await?;
    for e in v.list("aweme_list").iter().filter_map(Entry::of) {
      if !works.iter().any(|w| w.id == e.id) {
        works.push(e);
      }
    }
    match v.u64("cursor") {
      Some(next) if v.bool("has_more") == Some(true) && next != cursor => cursor = next,
      _ if v.bool("has_more") == Some(true) => {
        return Err(Error::upstream(
          "Douyin said more works follow but gave no place to go on from",
        ));
      }
      _ => return Ok(works),
    }
  }
  Err(Error::unsupported(format!(
    "collections of more than {} works",
    ITEM_PAGES * 50
  )))
}

/// One of the account's works (`item/info`, as the edit page loads it),
/// held to what the 合集 page's work picker refuses as far as it tells:
/// works others cannot see, works in another collection (`mix` is the
/// one they join).
async fn work(creator: &Creator<'_>, id: &str, mix: Option<&str>) -> Result<Entry> {
  let info = creator.get(ITEM_INFO).arg("item_id", id).send().await?;
  let aweme = info.at("aweme");
  if aweme.is_null() {
    return Err(Error::not_found(format!(
      "no work {id} on this account's creator platform"
    )));
  }
  let refuse = |why: String| Err(Error::new(ErrorCode::PermissionDenied, why));
  match aweme.i64("status.private_status") {
    Some(1) => {
      return refuse(format!(
        "work {id} is private (私密作品); a collection takes public works only: \
         `atp douyin edit {id} --visibility public` first (it uses no edit)"
      ));
    }
    Some(2) => {
      return refuse(format!(
        "work {id} is for friends only (好友可见作品); a collection takes public works \
         only: `atp douyin edit {id} --visibility public` first (it uses no edit)"
      ));
    }
    _ => {}
  }
  let other = aweme
    .first_str(&["mix_info.mix_id_str", "mix_info.mix_id"])
    .filter(|other| other != "0" && Some(other.as_str()) != mix);
  if let Some(other) = other {
    let name = aweme.str("mix_info.mix_name").unwrap_or(other);
    return refuse(format!(
      "work {id} is in the collection 「{name}」 (已加入其它合集); a work is in one \
       collection at most: take it out there first with `mix-remove`"
    ));
  }
  Ok(Entry::with_id(id.to_owned(), aweme))
}

/// The works given, as aweme ids (`#N` after a list of posts too), each
/// once.
async fn work_ids(api: &Api, works: &[String]) -> Result<Vec<String>> {
  let mut ids: Vec<String> = Vec::with_capacity(works.len());
  for arg in works {
    let id = refs::aweme(api, &api.ctx.post_ref(arg)?).await?;
    if ids.contains(&id) {
      return Err(Error::input(format!("work {id} is given twice")));
    }
    ids.push(id);
  }
  Ok(ids)
}

/// The title as the page takes it: 2 to 20 characters, no emoji.
fn checked_title(title: &str) -> Result<String> {
  let title = title.trim();
  if title.chars().any(is_emoji) {
    return Err(Error::input(
      "a collection's title takes no emoji (合集标题不允许输入emoji)",
    ));
  }
  if !TITLE.contains(&js_len(title)) {
    return Err(Error::input(
      "a collection's title takes 2 to 20 characters (合集标题字数要求2-20字)",
    ));
  }
  Ok(title.to_owned())
}

/// The description as the page takes it: on one line (its box drops line
/// breaks), 5 to 200 characters; an empty one is none.
fn checked_desc(desc: Option<&str>) -> Result<Option<String>> {
  let Some(desc) = desc else {
    return Ok(None);
  };
  let desc: String = desc.chars().filter(|c| *c != '\n' && *c != '\r').collect();
  let desc = desc.trim();
  if !desc.is_empty() && !DESC.contains(&js_len(desc)) {
    return Err(Error::input(
      "a collection's description takes 5 to 200 characters (合集简介字数要求5-200字)",
    ));
  }
  Ok(Some(desc.to_owned()))
}

/// What the page's cover picker refuses (other formats, over 5 MB, under
/// 100×100), and a note when the image is far from square: the page
/// crops it to 1:1, ATP sends it as it is.
async fn check_cover(path: &Path) -> Result<Vec<String>> {
  let image = atp_core::file::Image::read(path).await?;
  if !matches!(image.mime, "image/jpeg" | "image/png") {
    return Err(Error::input(format!(
      "{}: a collection's cover is a JPG or PNG image",
      path.display()
    )));
  }
  if image.data.len() as u64 > COVER_BYTES {
    return Err(Error::input(format!(
      "{}: a collection's cover takes 5 MB at most",
      path.display()
    )));
  }
  let (width, height) = image.size();
  if width == 0 || height == 0 {
    return Ok(Vec::new());
  }
  if width < COVER_MIN || height < COVER_MIN {
    return Err(Error::input(format!(
      "{}: a collection's cover is 100×100 at least",
      path.display()
    )));
  }
  let off = width as f64 / height as f64;
  if (0.9..=1.1).contains(&off) {
    return Ok(Vec::new());
  }
  Ok(vec![format!(
    "{} is {width}×{height}; it goes uncropped (the page crops covers to 1:1, 1080×1080 shows best)",
    path.display()
  )])
}

async fn upload_cover(api: &Api, creator: &Creator<'_>, path: &Path) -> Result<String> {
  note("uploading the cover");
  let up = Uploader::new(creator, &users::uid(api).await?).await?;
  Ok(up.image(path).await?.uri)
}

/// `mix/create/` as the page sends it (`qs`, `mix_id` null for a new
/// collection).
fn create_form(title: &str, desc: &str, cover: &str, ids: &[String]) -> Form {
  form(&[
    ("mix_id", ""),
    ("mix_name", title),
    ("mix_desc", desc),
    ("mix_cover", cover),
    ("item_ids", &ids.join(",")),
  ])
}

/// `mix/update/`: every field, changed or not.
fn update_form(mix: &Mix, title: &str, desc: &str, cover: &str) -> Form {
  form(&[
    ("mix_name", title),
    ("mix_desc", desc),
    ("mix_cover", cover),
    ("mix_id", &mix.id),
  ])
}

/// `mix/item/editlist/`: the whole new list in order, and the works it
/// adds and drops against the old one.
fn edit_list_form(id: &str, old: &[Entry], new: &[&Entry]) -> Form {
  let before: HashSet<&str> = old.iter().map(|e| e.id.as_str()).collect();
  let after: HashSet<&str> = new.iter().map(|e| e.id.as_str()).collect();
  let join = |ids: Vec<&str>| ids.join(",");
  let all = join(new.iter().map(|e| e.id.as_str()).collect());
  let added = join(
    new
      .iter()
      .map(|e| e.id.as_str())
      .filter(|id| !before.contains(id))
      .collect(),
  );
  let dropped = join(
    old
      .iter()
      .map(|e| e.id.as_str())
      .filter(|id| !after.contains(id))
      .collect(),
  );
  form(&[
    ("mix_id", id),
    ("item_ids", &all),
    ("add_item_ids", &added),
    ("del_item_ids", &dropped),
  ])
}

fn form(pairs: &[(&str, &str)]) -> Form {
  pairs
    .iter()
    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
    .collect()
}

/// The cover the page takes from a work for a collection without one:
/// the optimized cover, the cover, the original one, then an image
/// post's first image.
fn aweme_cover(aweme: &Value) -> String {
  aweme
    .first_str(&[
      "video.optimized_cover.uri",
      "video.cover.uri",
      "video.origin_cover.uri",
      "Cover.uri",
      "cover.uri",
      "images.0.uri",
    ])
    .unwrap_or_default()
}

/// Notes on a collection of `count` works (Douyin's rule: those of
/// fewer than 2 show to their author only).
fn count_notes(count: usize) -> Vec<&'static str> {
  match count {
    0 => vec!["with no works it is 待发布 (not published) until works are added (`mix-add`)"],
    n if n < SHOWN_MIN => {
      vec!["with fewer than 2 works only you see it (Douyin's rule); others see it once it has 2"]
    }
    _ => Vec::new(),
  }
}

fn status_name(code: i64) -> &'static str {
  match code {
    0 => "待发布 (not published)",
    REVIEWING => "审核中 (in review)",
    2 => "已发布 (published)",
    3 => "未通过 or 仅自己可见 (only you see it)",
    4 => "deleted",
    MADE_BY_DOUYIN => "made by Douyin",
    6 => "live replays (直播回放)",
    _ => "unknown",
  }
}

fn file_row(path: &Path) -> String {
  let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
  format!("{} ({} KB)", path.display(), bytes.div_ceil(1024))
}

/// Emoji the page's title box strips: pictographs, symbols, dingbats,
/// flags and the joiners between them.
fn is_emoji(c: char) -> bool {
  matches!(
    u32::from(c),
    0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0xFE0F | 0x200D | 0x20E3
  )
}

fn js_len(s: &str) -> usize {
  s.encode_utf16().count()
}

fn url(id: &str) -> String {
  format!("{WWW}/collection/{id}")
}

fn too_many() -> Error {
  Error::input(format!(
    "one change adds {ADD_MAX} works at most (单次添加进合集的视频不可以超过{ADD_MAX}个); add the rest in another"
  ))
}

fn not_in(id: &str) -> Error {
  Error::input(format!("work {id} is not in this collection"))
}

fn not_found(arg: &str) -> Error {
  Error::not_found(format!(
    "no collection of yours is {arg}; give its id, its douyin.com/collection/… link or its exact title"
  ))
}

async fn pause(api: &Api) {
  api
    .ctx
    .http
    .pause(Duration::from_secs(2), Duration::from_secs(6))
    .await;
}
