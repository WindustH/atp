//! Chapters (分段章节) of videos: what the player shows (`view_points` of
//! the player info, type 2) and what an uploader sets through the
//! player's editor (www.bilibili.com/web/player, `scene=chapter`; bundle
//! `editor.*.js`, `Ub.setCards`): `allcards` lists the cards of a part,
//! `card/submit` replaces its chapters (cards of type 2, an empty list
//! removes them), and `card/pre/chapter/submit` gives chapters to a file
//! just uploaded, before its archive is submitted. Chapters belong to
//! one part (cid). The editor's rules: videos over a minute, 2 to 10
//! chapters, the first at 0:00, each at least 5 seconds, titles of 16
//! characters at most.

use atp_core::chapters::{self, Chapter, Rules};
use atp_core::{Error, ErrorCode, Result, Value, ValueExt, json};

use crate::api::Api;

const PLAYER: &str = "https://api.bilibili.com/x/player/wbi/v2";
const CARDS: &str = "https://member.bilibili.com/x/web/allcards";
const SUBMIT: &str = "https://member.bilibili.com/x/web/card/submit";
const PRE_SUBMIT: &str = "https://member.bilibili.com/x/web/card/pre/chapter/submit";
/// The editor page that sends them (an iframe of the creator center).
const EDITOR: &str =
  "https://www.bilibili.com/web/player/index.html?source_type=editor&scene=chapter";
/// The card type of chapters (`view_points` type too).
const CHAPTERS: i64 = 2;

pub const RULES: Rules = Rules {
  platform: "Bilibili",
  min_count: 2,
  max_count: 10,
  first_at_zero: true,
  min_length: 5,
  title_max: 16,
  min_video: 61,
};

/// The chapters a part has now, for `edit`: as set by its uploader, and
/// whether the app shows them by default (`permanent`).
#[derive(Debug, Clone, Default)]
pub struct Current {
  pub chapters: Vec<Chapter>,
  pub permanent: Option<bool>,
}

/// A change of one part's chapters, checked: what it has and what it gets.
#[derive(Debug, Clone)]
pub struct Change {
  pub aid: u64,
  pub cid: u64,
  /// The part's length, in seconds.
  pub duration: u32,
  pub old: Vec<Chapter>,
  pub new: Vec<Chapter>,
  pub permanent: Option<bool>,
}

/// The change to `new` chapters (empty: none) of `part`, a part of the
/// archive as the creator center's `archive/view` gives it.
pub async fn change(api: &Api, archive: &Value, part: &Value, new: &[Chapter]) -> Result<Change> {
  if archive.bool("chapter.can_edit") == Some(false) {
    let why = archive
      .str("chapter.can_edit_msg")
      .unwrap_or_else(|| "this video's chapters cannot change".into());
    return Err(Error::new(
      ErrorCode::PermissionDenied,
      format!("Bilibili: {why}"),
    ));
  }
  let (Some(aid), Some(cid)) = (archive.u64("aid"), part.u64("cid")) else {
    return Err(Error::upstream("Bilibili sent the video without its ids"));
  };
  let duration = part.u64("duration").unwrap_or(0) as u32;
  if !new.is_empty() {
    if duration == 0 {
      return Err(Error::upstream(
        "Bilibili sent the part without its length, which the last chapter ends at",
      ));
    }
    chapters::check(new, &RULES, Some(duration))?;
  }
  let current = current(api, aid, cid).await?;
  if new.is_empty() && current.chapters.is_empty() {
    return Err(Error::input("this video has no chapters to remove"));
  }
  Ok(Change {
    aid,
    cid,
    duration,
    old: current.chapters,
    new: new.to_vec(),
    permanent: current.permanent,
  })
}

/// The `chapters` of `read` (`Post.extra`), from the player info of a
/// part; `None` without chapters or when the player info fails.
pub async fn read(api: &Api, aid: u64, cid: u64) -> Option<Value> {
  let data = api
    .get(PLAYER)
    .arg("aid", aid)
    .arg("cid", cid)
    .arg("isGaiaAvoided", "false")
    .arg("web_location", 1315873)
    .wbi()
    .send()
    .await
    .map_err(|e| tracing::debug!("player info for chapters: {e}"))
    .ok()?;
  let points: Vec<&Value> = data
    .list("view_points")
    .iter()
    .filter(|p| p.i64("type") == Some(CHAPTERS))
    .collect();
  let list: Vec<(String, f64)> = points
    .iter()
    .map(|p| {
      let title = p.str("content").unwrap_or_default();
      (title, p.f64("from").unwrap_or(0.0))
    })
    .collect();
  let end = points.last().and_then(|p| p.f64("to"));
  (!list.is_empty()).then(|| chapters::value(&list, end))
}

/// The chapters a part has now, as its uploader set them.
pub async fn current(api: &Api, aid: u64, cid: u64) -> Result<Current> {
  let data = api
    .get(CARDS)
    .arg("aid", aid)
    .arg("cid", cid)
    .send()
    .await?;
  let Some(catalog) = data
    .list("catalog")
    .iter()
    .find(|c| c.i64("type") == Some(CHAPTERS))
    .cloned()
  else {
    return Ok(Current::default());
  };
  let chapters = catalog
    .list("cards")
    .iter()
    .map(|c| Chapter {
      start: c.f64("from").unwrap_or(0.0).round() as u32,
      title: c.str("content").unwrap_or_default(),
    })
    .collect();
  Ok(Current {
    chapters,
    permanent: catalog.bool("permanent"),
  })
}

/// The cards of chapters, as the editor sends them (`cards` / `chapters`):
/// each from its start to the next one, the last to the end of the part.
pub fn cards(list: &[Chapter], duration: u32) -> String {
  let cards: Vec<Value> = list
    .iter()
    .zip(chapters::ends(list, duration))
    .map(|(c, end)| json!({ "from": c.start, "to": end, "content": c.title }))
    .collect();
  Value::from(cards).to_string()
}

/// Replace the chapters of a published part; an empty list removes them.
pub async fn submit(api: &Api, change: &Change, bvid: &str) -> Result<()> {
  let mut call = api
    .post(SUBMIT)
    .arg("aid", change.aid)
    .arg("cid", change.cid)
    .arg("type", CHAPTERS)
    .arg("cards", cards(&change.new, change.duration));
  if let Some(permanent) = change.permanent {
    call = call.arg("permanent", permanent);
  }
  call
    .header("referer", format!("{EDITOR}&from=publish_edit&bvid={bvid}"))
    .send()
    .await?;
  Ok(())
}

/// Give chapters to a file just uploaded (its cid), before its archive
/// is submitted with `has_chapter`.
pub async fn submit_new(api: &Api, cid: u64, list: &[Chapter], duration: u32) -> Result<()> {
  api
    .post(PRE_SUBMIT)
    .arg("cid", cid)
    .arg("chapters", cards(list, duration))
    .header("referer", format!("{EDITOR}&from=publish&cid={cid}"))
    .send()
    .await?;
  Ok(())
}
