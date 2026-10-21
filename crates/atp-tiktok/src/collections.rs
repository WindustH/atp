//! Your favorites collections themselves, as the Favorites tab's "Create
//! new collection" and a collection's menu ("Make public", "Edit name",
//! "Delete") change them: `/api/collection/create/` with `collectionName`
//! and `collectionStatus`, `/api/collection/modify_info/` with both and the
//! `collectionId` (the menu sends the name and the visibility every time),
//! and `/api/collection/delete/`. The parameters go in the query and the
//! body is empty, so they are signed like the crate's other writes. The
//! pages run TikTok's name check before naming a collection, and so does
//! ATP.

use atp_core::bulk;
use atp_core::{Action, Error, FolderSpec, Result, Value, ValueExt};

use crate::api::{Api, WWW};
use crate::{parse, playlists, refs, users, videos};

const CREATE: &str = "/api/collection/create/";
const MODIFY: &str = "/api/collection/modify_info/";
const DELETE: &str = "/api/collection/delete/";
const LIST: &str = "/api/user/collection_list/";

/// `collectionStatus`: only you see it, or it shows on your profile.
const PRIVATE: i64 = 1;
const PUBLIC: i64 = 3;
/// The longest name the page takes, in JavaScript string length.
const NAME_MAX: usize = 30;
/// Pages of collections (30 each) a lookup goes through.
const LIST_PAGES: usize = 20;

pub async fn create(api: &Api, spec: &FolderSpec) -> Result<Action> {
  api.require_login()?;
  let name = checked_name(spec.name.as_deref().unwrap_or_default())?;
  playlists::check_name(api, &name).await?;
  let status = if spec.private == Some(true) {
    PRIVATE
  } else {
    PUBLIC
  };
  api.write_pause().await;
  let params = [
    ("collectionName", name.clone()),
    ("collectionStatus", status.to_string()),
  ];
  let v = api.post(CREATE, &params).await?;
  let action = Action::done("folder-create", &name).with_message(visibility(status));
  let Some(id) = v.first_str(&["collectionIdString", "collectionId"]) else {
    return Ok(action);
  };
  Ok(match api.ctx.extra("handle") {
    Some(handle) => action.with_url(url(&handle, &name, &id)).with_id(id),
    None => action.with_id(id),
  })
}

/// Rename a collection, or make it public or private: what is not given
/// is read from the collection, since the menu sends both.
pub async fn edit(api: &Api, folder: &str, spec: &FolderSpec) -> Result<Action> {
  api.require_login()?;
  let id = refs::collection_id(folder)?;
  let current = find(api, &id).await?;
  let old_name = current
    .first_str(&["name", "collectionName"])
    .unwrap_or_default();
  let old_status = current.i64("status");
  let name = match &spec.name {
    Some(name) => checked_name(name)?,
    None => old_name.clone(),
  };
  let status = match (spec.private, old_status) {
    (Some(true), _) => PRIVATE,
    (Some(false), _) => PUBLIC,
    (None, Some(status)) => status,
    (None, None) => {
      return Err(Error::upstream(
        "TikTok did not say whether the collection is public; give --private or --public too",
      ));
    }
  };
  if name == old_name && Some(status) == old_status {
    return Err(Error::input(
      "nothing changes: the collection is like this already",
    ));
  }
  if name != old_name {
    playlists::check_name(api, &name).await?;
  }
  api.write_pause().await;
  let params = [
    ("collectionId", id.clone()),
    ("collectionName", name.clone()),
    ("collectionStatus", status.to_string()),
  ];
  api.post(MODIFY, &params).await?;
  let message = format!("{name}, {}", visibility(status));
  Ok(Action::done("folder-edit", &id).with_message(message))
}

/// Delete collections, one call each; the videos stay saved.
pub async fn delete(api: &Api, folders: &[String]) -> Result<Vec<Action>> {
  api.require_login()?;
  bulk::each("folder-delete", folders, async |folder| {
    let id = refs::collection_id(folder)?;
    api.write_pause().await;
    api.post(DELETE, &[("collectionId", id.clone())]).await?;
    Ok(Action::done("folder-delete", id))
  })
  .await
}

/// One of your collections as your Favorites tab lists them (private ones
/// too, as the page asks for its own).
async fn find(api: &Api, id: &str) -> Result<Value> {
  let (sec, _) = users::ids(api, None).await?;
  let mut cursor = "0".to_owned();
  for _ in 0..LIST_PAGES {
    let params = [
      ("secUid", sec.clone()),
      ("count", "30".to_owned()),
      ("cursor", cursor.clone()),
      ("publicOnly", "false".to_owned()),
    ];
    let v = api.get(LIST, &params).await?;
    let found = v
      .list("collectionList")
      .iter()
      .find(|c| c.first_str(&["collectionId", "id"]).as_deref() == Some(id));
    if let Some(found) = found {
      return Ok(found.clone());
    }
    match videos::next_cursor(&v) {
      Some(next) if next != cursor => cursor = next,
      _ => break,
    }
  }
  Err(Error::not_found(format!(
    "no collection {id} among yours; `atp tiktok collections` lists them"
  )))
}

/// The name as the pages take it: trimmed, 1 to 30 characters.
fn checked_name(name: &str) -> Result<String> {
  let name = name.trim();
  if name.encode_utf16().count() > NAME_MAX {
    return Err(Error::input(format!(
      "a collection name takes {NAME_MAX} characters at most (Character limit exceeded)"
    )));
  }
  Ok(name.to_owned())
}

fn visibility(status: i64) -> &'static str {
  match status {
    PRIVATE => "private",
    PUBLIC => "public, shown on your profile",
    _ => "visibility unknown",
  }
}

fn url(handle: &str, name: &str, id: &str) -> String {
  format!("{WWW}/@{handle}/collection/{}-{id}", parse::slug(name))
}
