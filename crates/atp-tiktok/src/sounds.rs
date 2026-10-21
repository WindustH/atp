//! The sounds you saved (Favorites → Sounds), listed, saved and unsaved
//! the way TikTok Studio's sound picker does: `/api/user/collect/music_list/`
//! with your secUid, and `/api/music/collect/` with the sound's id and
//! `action` 1 (save) or 2 (unsave).

use atp_core::bulk;
use atp_core::{Action, Collection, Page, PageReq, Result, ValueExt};

use crate::api::{Api, WWW};
use crate::videos::next_cursor;
use crate::{parse, refs, users};

const SAVED: &str = "/api/user/collect/music_list/";
const COLLECT: &str = "/api/music/collect/";
/// The picker asks for 20 at a time.
const SAVED_PAGE: usize = 20;

/// Your saved sounds, latest saved first.
pub async fn saved(api: &Api, req: &PageReq) -> Result<Page<Collection>> {
  api.require_login()?;
  let (sec, _) = users::ids(api, None).await?;
  let params = vec![
    ("secUid", sec),
    ("count", req.size_within(SAVED_PAGE).to_string()),
    ("cursor", req.cursor.clone().unwrap_or_else(|| "0".into())),
  ];
  let v = api.get(SAVED, &params).await?;
  let items: Vec<Collection> = v
    .list("musicList")
    .iter()
    .filter_map(parse::sound)
    .collect();
  let next = next_cursor(&v).filter(|_| !items.is_empty());
  Ok(Page::new(items, next))
}

/// Save sounds, or unsave them with `undo`, one call each.
pub async fn save(api: &Api, sounds: &[String], undo: bool) -> Result<Vec<Action>> {
  api.require_login()?;
  let ids = sounds
    .iter()
    .map(|s| refs::music_id(s))
    .collect::<Result<Vec<_>>>()?;
  let (name, action) = if undo {
    ("unsave-music", "2")
  } else {
    ("save-music", "1")
  };
  bulk::each(name, &ids, async |id| {
    api.write_pause().await;
    let params = [("musicId", id.to_owned()), ("action", action.into())];
    api.post(COLLECT, &params).await?;
    Ok(Action::done(name, id).with_url(format!("{WWW}/music/x-{id}")))
  })
  .await
}
