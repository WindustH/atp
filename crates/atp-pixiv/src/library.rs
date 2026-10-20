//! With a login: bookmarked artworks (yours, public and private, or a
//! user's public ones) with their bookmark tags as folders, and the new
//! works of the artists you follow.
//!
//! A folder is a bookmark tag: `TAG` for the public bookmarks with it,
//! `private` for every private bookmark, `private:TAG` for the private
//! ones with it. `collections` lists them with these ids.

use atp_core::{Collection, Error, Page, PageReq, Post, Result, ValueExt, json, text};

use crate::api::{Api, Query};
use crate::parse;
use crate::refs;

/// `feed --type`: new artworks (the default) or new novels.
pub const FEED_KINDS: &[&str] = &["artworks", "novels"];
/// The most bookmarks a page asks for.
const PAGE: usize = 48;
/// The folder of private bookmarks, and the prefix of its tags.
const PRIVATE: &str = "private";

/// The user whose library to read: the one named, or the account.
fn owner(api: &Api, user: Option<&str>) -> Result<String> {
  match user {
    Some(user) => refs::user(user),
    None => api.need_login(),
  }
}

/// `(rest, tag)` of a folder.
fn folder_of(folder: Option<&str>) -> (&'static str, String) {
  match folder.map(str::trim) {
    None | Some("") => ("show", String::new()),
    Some(PRIVATE) => ("hide", String::new()),
    Some(other) => match other.strip_prefix("private:") {
      Some(tag) => ("hide", tag.to_owned()),
      None => ("show", other.to_owned()),
    },
  }
}

pub async fn favorites(
  api: &Api,
  user: Option<&str>,
  folder: Option<&str>,
  page: &PageReq,
) -> Result<Page<Post>> {
  let uid = owner(api, user)?;
  let (rest, tag) = folder_of(folder);
  if rest == "hide" && api.uid().as_deref() != Some(uid.as_str()) {
    return Err(Error::input("private bookmarks are only your own"));
  }
  let offset = page.number_or(0);
  let query: Query = vec![
    ("tag", tag),
    ("offset", offset.to_string()),
    ("limit", page.size_within(PAGE).to_string()),
    ("rest", rest.to_owned()),
  ];
  let v = api
    .get(&format!("/ajax/user/{uid}/illusts/bookmarks"), query)
    .await?;
  let rows = v.list("works");
  let posts = rows
    .iter()
    .filter(|row| api.logged_in() || !parse::restricted(row))
    .map(parse::artwork)
    .collect();
  let seen = offset + rows.len() as u64;
  let next = (!rows.is_empty() && seen < v.u64("total").unwrap_or(0)).then(|| seen.to_string());
  Ok(Page::new(posts, next))
}

/// The bookmark tags of a user (with yours, the private ones too).
pub async fn collections(
  api: &Api,
  user: Option<&str>,
  page: &PageReq,
) -> Result<Page<Collection>> {
  let uid = owner(api, user)?;
  let v = api
    .get(
      &format!("/ajax/user/{uid}/illusts/bookmark/tags"),
      Vec::new(),
    )
    .await?;
  let mut tags = Vec::new();
  for (rest, prefix) in [("public", ""), ("private", "private:")] {
    for row in v.list(rest) {
      let Some(tag) = row.str("tag") else { continue };
      let mut folder = Collection {
        id: format!("{prefix}{tag}"),
        kind: "tag".into(),
        url: Some(format!(
          "{}/bookmarks/artworks/{}",
          parse::user_url(&uid),
          text::encode_uri_component(&tag)
        )),
        items: row.count("cnt"),
        raw: Some(row.clone()),
        name: tag,
        ..Collection::default()
      };
      if !prefix.is_empty() {
        folder.extra.insert("private".into(), json!(true));
        folder.url = None;
      }
      tags.push(folder);
    }
  }
  Ok(atp_core::paging::slice(tags, page))
}

/// The new works of the artists you follow, newest first.
pub async fn feed(api: &Api, kind: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  api.need_login()?;
  let (path, section) = match kind {
    Some("novels") => ("novel", "novel"),
    _ => ("illust", "illust"),
  };
  let no = page.number_or(1);
  let query: Query = vec![("p", no.to_string()), ("mode", "all".to_owned())];
  let v = api
    .get(&format!("/ajax/follow_latest/{path}"), query)
    .await?;
  let rows = v.list(&format!("thumbnails.{section}"));
  let posts = rows
    .iter()
    .map(|row| match section {
      "novel" => parse::novel(row),
      _ => parse::artwork(row),
    })
    .collect();
  let last = v.bool("page.isLastPage").unwrap_or(true);
  let next = (!last && !rows.is_empty()).then(|| (no + 1).to_string());
  Ok(Page::new(posts, next))
}
