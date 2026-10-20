//! Actions of the login, sent as the site's pages send them with the
//! token of the front page (`x-csrf-token`): liking an artwork (いいね!,
//! which the site cannot take back), bookmarking it and removing the
//! bookmark, following and unfollowing an artist. Novels are not liked
//! or bookmarked here.

use atp_core::{Action, Error, ErrorCode, Result, ValueExt, json};

use crate::api::Api;
use crate::refs::{self, WorkRef};
use crate::{parse, works};

/// The artwork of an action; novels are refused.
fn artwork(arg: &str, action: &str) -> Result<String> {
  match refs::work(arg)? {
    WorkRef::Artwork(id) => Ok(id),
    WorkRef::Novel(_) => Err(Error::new(
      ErrorCode::UnsupportedOperation,
      format!("ATP does not {action} novels on pixiv"),
    )),
  }
}

pub async fn like(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  if undo {
    return Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "pixiv's likes cannot be taken back",
    ));
  }
  let id = artwork(arg, "like")?;
  let v = api
    .post_json("/ajax/illusts/like", &json!({ "illust_id": id }))
    .await?;
  let action = Action::done("like", parse::artwork_url(&id));
  Ok(match v.bool("is_liked") {
    Some(true) => action.with_message("liked before"),
    _ => action,
  })
}

/// Bookmark an artwork (publicly, or privately with the folder `private`;
/// a folder `TAG` or `private:TAG` adds that bookmark tag), or remove the
/// bookmark.
pub async fn favorite(api: &Api, arg: &str, folder: Option<&str>, undo: bool) -> Result<Action> {
  let id = artwork(arg, "bookmark")?;
  let url = parse::artwork_url(&id);
  if undo {
    return unfavorite(api, &id, url).await;
  }
  let folder = folder.map(str::trim).unwrap_or_default();
  let (restrict, tag) = match folder.strip_prefix("private") {
    Some("") => (1, ""),
    Some(rest) if rest.starts_with(':') => (1, &rest[1..]),
    _ => (0, folder),
  };
  let tags: Vec<&str> = [tag].into_iter().filter(|t| !t.is_empty()).collect();
  let body = json!({ "illust_id": id, "restrict": restrict, "comment": "", "tags": tags });
  let v = api.post_json("/ajax/illusts/bookmarks/add", &body).await?;
  let mut action = Action::done("favorite", url);
  if let Some(bookmark) = v.str("last_bookmark_id") {
    action = action.with_id(bookmark);
  }
  Ok(action)
}

/// Remove a bookmark, found in the artwork's own data.
async fn unfavorite(api: &Api, id: &str, url: String) -> Result<Action> {
  api.need_login()?;
  let v = works::artwork_json(api, id).await?;
  let Some(bookmark) = v.str("bookmarkData.id") else {
    return Ok(Action::done("unfavorite", url).with_message("not bookmarked"));
  };
  api
    .post_form(
      "/ajax/illusts/bookmarks/delete",
      &[("bookmark_id", bookmark)],
    )
    .await?;
  Ok(Action::done("unfavorite", url))
}

pub async fn follow(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let uid = refs::user(arg)?;
  let url = parse::user_url(&uid);
  if undo {
    let form = [
      ("mode", "del".to_owned()),
      ("type", "bookuser".to_owned()),
      ("id", uid),
    ];
    api.post_form("/rpc_group_setting.php", &form).await?;
    return Ok(Action::done("unfollow", url));
  }
  let form = [
    ("mode", "add".to_owned()),
    ("type", "user".to_owned()),
    ("user_id", uid),
    ("tag", String::new()),
    ("restrict", "0".to_owned()),
    ("format", "json".to_owned()),
  ];
  api.post_form("/bookmark_add.php", &form).await?;
  Ok(Action::done("follow", url))
}
