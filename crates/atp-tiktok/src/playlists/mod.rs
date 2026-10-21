//! Your playlists, as the profile's Playlists tab and a playlist's page
//! keep them: one made from your posts (`playlist/create_with_items/`), its
//! name (`playlist/update/`), deleting it (`playlist/delete/`), its posts
//! added, taken out and put in order (`playlist/modify_items/` with
//! `addItemIds`, `deleteItemsIds` or `commitItemIds`), and the order of the
//! playlists on the profile (`playlist/modify_order/`). These calls carry
//! a form body, so TikTok's page makes them ([`crate::tab`]).
//!
//! What the pages check is read and checked first ([`lookup`]): the
//! account may make playlists (`canExpPlaylist`), the name passes TikTok's
//! name check, a post is yours, public and in no other playlist. Nothing
//! goes out before the person has seen the change and said yes.

mod lookup;

use atp_core::cli::Change;
use atp_core::{Action, Error, ErrorCode, Result, Value, ValueExt};

use self::lookup::{Me, matching, not_found, post_ids};
use crate::api::{self, Api};
use crate::tab;

const CREATE: &str = "/api/playlist/create_with_items/";
const UPDATE: &str = "/api/playlist/update/";
const DELETE: &str = "/api/playlist/delete/";
const ITEMS: &str = "/api/playlist/modify_items/";
const ORDER: &str = "/api/playlist/modify_order/";
const NAME_CHECK: &str = "/api/playlist/name_check";

/// The longest name the page takes, in JavaScript string length.
const NAME_MAX: usize = 25;
/// Posts the page lets one pick for a playlist at most
/// (`playlist_webapp_toast_error_vidlimit`).
const POSTS_MAX: usize = 100;

/// Status codes of the playlist calls.
const CREATE_RISK: i64 = 206_001;
const VISIBLE_TO_CREATOR: i64 = 206_002;
const ADD_RISK: i64 = 206_003;
const GONE: &[i64] = &[10_233, 10_243, 206_005];

const KEPT: &str = "the posts stay on your account (Your posts will still be saved)";

/// Create a playlist of `posts`, in that order.
pub async fn create(api: &Api, title: &str, posts: &[String], yes: bool) -> Result<Action> {
  let name = checked_name(title)?;
  let ids = post_ids(api, posts).await?;
  if ids.len() > POSTS_MAX {
    return Err(too_many());
  }
  let me = Me::load(api).await?;
  if me.can_create == Some(false) {
    return Err(Error::new(
      ErrorCode::PermissionDenied,
      "TikTok does not offer playlists to this account: its profile has no \
       \"Create a playlist\" (canExpPlaylist is false)",
    ));
  }
  check_name(api, &name).await?;
  let mut change = Change::new("TikTok playlist, new:");
  change.row("name", &name);
  change.row("posts", &ids.len().to_string());
  for (i, id) in ids.iter().enumerate() {
    let entry = me.post(api, id, None).await?;
    change.row(&format!("post {}", i + 1), &entry.row());
  }
  change.row(
    "",
    "a post is in one playlist at most; the playlist shows on your profile",
  );
  change.confirm(yes, "create this playlist on TikTok")?;
  let form = [("name", name.clone()), ("itemIds", ids.join(","))];
  let v = send(api, &me, CREATE, &form, true).await?;
  let action = Action::done("playlist-create", &name);
  let Some(id) = v.first_str(&["playlistId", "playlist_id", "mixId"]) else {
    return Ok(action.with_message("created; TikTok named no id (`playlists` lists it)"));
  };
  let action = action.with_url(me.url(&name, &id)).with_id(id);
  Ok(match v.i64("statusCode") {
    Some(VISIBLE_TO_CREATOR) => action.with_message("created; TikTok shows it to you only for now"),
    _ => action,
  })
}

/// Rename a playlist.
pub async fn rename(api: &Api, arg: &str, title: &str, yes: bool) -> Result<Action> {
  let name = checked_name(title)?;
  let me = Me::load(api).await?;
  let list = me.playlist(api, arg).await?;
  if list.name == name {
    return Err(Error::input(
      "nothing changes: the playlist has this name already",
    ));
  }
  check_name(api, &name).await?;
  let mut change = list.change();
  change.field("name", &list.name, &name);
  change.confirm(yes, "rename this playlist on TikTok")?;
  let form = [("playlistId", list.id.clone()), ("name", name.clone())];
  send(api, &me, UPDATE, &form, true).await?;
  Ok(list.done("playlist-edit", &me, &name))
}

/// Delete a playlist; its posts stay.
pub async fn delete(api: &Api, arg: &str, yes: bool) -> Result<Action> {
  let me = Me::load(api).await?;
  let list = me.playlist(api, arg).await?;
  let mut change = list.change();
  change.row("", KEPT);
  change.row("", "a deleted playlist cannot be recovered");
  change.confirm(yes, "delete this playlist on TikTok")?;
  send(api, &me, DELETE, &[("playlistId", list.id.clone())], false).await?;
  Ok(Action::done("playlist-delete", &list.id).with_id(&list.id))
}

/// Add posts to a playlist.
pub async fn add(api: &Api, arg: &str, posts: &[String], yes: bool) -> Result<Action> {
  let ids = post_ids(api, posts).await?;
  let me = Me::load(api).await?;
  let list = me.playlist(api, arg).await?;
  let total = list.count as usize + ids.len();
  if total > POSTS_MAX {
    return Err(too_many());
  }
  let mut change = list.change();
  change.field("posts", &list.count.to_string(), &total.to_string());
  for (i, id) in ids.iter().enumerate() {
    let entry = me.post(api, id, Some(&list.id)).await?;
    change.row(if i == 0 { "add" } else { "" }, &entry.row());
  }
  change.row(
    "",
    "the playlist's page shows added posts first; `playlist-sort` reorders",
  );
  change.confirm(yes, "add these posts to the playlist on TikTok")?;
  let form = [
    ("playlistId", list.id.clone()),
    ("addItemIds", ids.join(",")),
  ];
  send(api, &me, ITEMS, &form, false).await?;
  Ok(list.done("playlist-add", &me, &list.name))
}

/// Take posts out of a playlist; they stay on the account.
pub async fn remove(api: &Api, arg: &str, posts: &[String], yes: bool) -> Result<Action> {
  let ids = post_ids(api, posts).await?;
  let me = Me::load(api).await?;
  let mut list = me.playlist(api, arg).await?;
  list.read_posts(api).await?;
  let mut change = list.change();
  for (i, id) in ids.iter().enumerate() {
    let at = list.position(id).ok_or_else(|| not_in(id))?;
    change.row(if i == 0 { "remove" } else { "" }, &list.row(at));
  }
  let left = list.posts.len() - ids.len();
  change.row("", &format!("{left} posts left"));
  change.row("", KEPT);
  change.confirm(yes, "take these posts out of the playlist on TikTok")?;
  let form = [
    ("playlistId", list.id.clone()),
    ("deleteItemsIds", ids.join(",")),
  ];
  send(api, &me, ITEMS, &form, false).await?;
  Ok(list.done("playlist-remove", &me, &list.name))
}

/// Put a playlist's posts in a new order: those given first, in that
/// order, then the others as they were.
pub async fn sort(api: &Api, arg: &str, posts: &[String], yes: bool) -> Result<Action> {
  let ids = post_ids(api, posts).await?;
  let me = Me::load(api).await?;
  let mut list = me.playlist(api, arg).await?;
  list.read_posts(api).await?;
  if let Some(id) = ids.iter().find(|id| list.position(id).is_none()) {
    return Err(not_in(id));
  }
  let old: Vec<&str> = list.posts.iter().map(|e| e.id.as_str()).collect();
  let given: Vec<&str> = ids.iter().map(String::as_str).collect();
  let new = reordered(&old, &given);
  let Some(last) = last_change(&old, &new) else {
    return Err(Error::input(
      "nothing changes: the posts are in this order already",
    ));
  };
  let mut change = list.change();
  let mut label = "order";
  for (at, id) in new.iter().enumerate().take(last + 1) {
    let was = list.position(id).unwrap_or_default();
    if was != at {
      let row = format!(
        "post {}: {} (was {})",
        at + 1,
        list.posts[was].row(),
        was + 1
      );
      change.row(label, &row);
      label = "";
    }
  }
  change.confirm(yes, "put the playlist's posts in this order on TikTok")?;
  // The page sends the new order up to the last post that moved.
  let form = [
    ("playlistId", list.id.clone()),
    ("commitItemIds", new[..=last].join(",")),
  ];
  send(api, &me, ITEMS, &form, false).await?;
  Ok(list.done("playlist-sort", &me, &list.name))
}

/// Put your playlists in a new order on your profile: those given first,
/// in that order, then the others as they were.
pub async fn order(api: &Api, args: &[String], yes: bool) -> Result<Action> {
  let me = Me::load(api).await?;
  let all = me.playlists(api).await?;
  let mut given: Vec<&str> = Vec::with_capacity(args.len());
  for arg in args {
    let found = matching(&all, arg).ok_or_else(|| not_found(arg))?;
    if given.contains(&found.id.as_str()) {
      return Err(Error::input(format!(
        "playlist {} is given twice",
        found.id
      )));
    }
    given.push(&found.id);
  }
  let old: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
  let new = reordered(&old, &given);
  let Some(last) = last_change(&old, &new) else {
    return Err(Error::input(
      "nothing changes: the playlists are in this order already",
    ));
  };
  let mut change = Change::new(format!("TikTok playlists of @{}:", me.handle));
  let mut label = "order";
  for (at, id) in new.iter().enumerate().take(last + 1) {
    let was = old.iter().position(|o| o == id).unwrap_or_default();
    if was != at {
      let row = format!(
        "playlist {}: {id} {} (was {})",
        at + 1,
        all[was].name,
        was + 1
      );
      change.row(label, &row);
      label = "";
    }
  }
  change.confirm(yes, "put your playlists in this order on TikTok")?;
  // As the page's "Change playlist order": the new order up to the last move.
  let form = [("commitMixIds", new[..=last].join(","))];
  send(api, &me, ORDER, &form, false).await?;
  Ok(Action::done("playlists-sort", format!("@{}", me.handle)).with_url(me.page()))
}

/// TikTok's name check, which the pages run before they name a playlist or
/// a favorites collection.
pub async fn check_name(api: &Api, name: &str) -> Result<()> {
  let v = api.get(NAME_CHECK, &[("name", name.to_owned())]).await?;
  if v.bool("isValid") == Some(false) {
    return Err(Error::input(format!(
      "TikTok refuses the name {name:?} (This name is inappropriate. Try again.)"
    )));
  }
  Ok(())
}

/// A playlist call made by the page; TikTok's refusals as errors.
async fn send(
  api: &Api,
  me: &Me,
  path: &str,
  form: &[(&str, String)],
  csrf: bool,
) -> Result<Value> {
  let v = tab::post(api, &me.page(), path, form, csrf).await?;
  let code = v
    .i64("statusCode")
    .or_else(|| v.i64("status_code"))
    .unwrap_or(0);
  let refused = |what: &str| {
    Err(Error::new(
      ErrorCode::PermissionDenied,
      format!("TikTok: {what} (status {code})"),
    ))
  };
  match code {
    0 | VISIBLE_TO_CREATOR => Ok(v),
    CREATE_RISK => refused("Couldn't create playlist"),
    ADD_RISK => refused("Couldn't add post to playlist"),
    c if GONE.contains(&c) => Err(Error::not_found(format!(
      "TikTok: the playlist is unavailable or deleted (status {code})"
    ))),
    _ => api::check(path, v),
  }
}

/// `old` with `first` moved to its front, in that order.
fn reordered<'a>(old: &[&'a str], first: &[&'a str]) -> Vec<&'a str> {
  let mut new = first.to_vec();
  new.extend(old.iter().filter(|id| !first.contains(id)));
  new
}

/// The last position whose entry moves, if any does. The pages send the
/// new order up to there; what follows stays as it is.
fn last_change(old: &[&str], new: &[&str]) -> Option<usize> {
  (0..new.len()).rev().find(|&i| old.get(i) != new.get(i))
}

/// The name as the pages take it: trimmed, 1 to 25 characters.
fn checked_name(name: &str) -> Result<String> {
  let name = name.trim();
  if name.is_empty() {
    return Err(Error::input("the playlist name is empty"));
  }
  if name.encode_utf16().count() > NAME_MAX {
    return Err(Error::input(format!(
      "a playlist name takes {NAME_MAX} characters at most (You can add up to 25 characters)"
    )));
  }
  Ok(name.to_owned())
}

fn too_many() -> Error {
  Error::input(format!(
    "a playlist takes {POSTS_MAX} posts at most here: the page lets one pick no more \
     (You can select up to {POSTS_MAX} posts at a time)"
  ))
}

fn not_in(id: &str) -> Error {
  Error::input(format!("post {id} is not in this playlist"))
}
