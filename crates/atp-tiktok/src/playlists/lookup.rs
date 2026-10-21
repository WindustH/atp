//! What the playlist commands read before they change anything: the
//! account, its playlists in the profile's order, a playlist's posts, and
//! posts held to what the page's post picker takes.

use atp_core::cli::Change;
use atp_core::text::{one_line, truncate};
use atp_core::{Action, Collection, Error, ErrorCode, PageReq, Post, Result, Value, ValueExt};

use crate::api::{Api, WWW};
use crate::{parse, refs, users, videos};

const LIST: &str = "/api/user/playlist/";
/// Pages of playlists (20 each) and of a playlist's posts (30 each) read.
const LIST_PAGES: usize = 20;
const POST_PAGES: usize = 20;

/// The logged-in account.
pub struct Me {
  pub handle: String,
  sec_uid: String,
  /// `canExpPlaylist` of the profile: whether TikTok offers it playlists.
  pub can_create: Option<bool>,
}

/// One of your playlists, with its posts in order once they are read.
pub struct Playlist {
  pub id: String,
  pub name: String,
  pub count: u64,
  pub posts: Vec<Entry>,
}

/// A post as a summary shows it.
pub struct Entry {
  pub id: String,
  label: String,
}

impl Me {
  pub async fn load(api: &Api) -> Result<Self> {
    api.require_login()?;
    let user = users::whoami(api).await?;
    let handle = user
      .handle
      .clone()
      .ok_or_else(|| Error::upstream("the session names no account"))?;
    let sec_uid = user
      .extra
      .get("sec_uid")
      .and_then(Value::as_str)
      .map(str::to_owned)
      .ok_or_else(|| Error::upstream("the profile carries no secUid"))?;
    let can_create = user
      .raw
      .as_ref()
      .and_then(|r| r.bool("user.canExpPlaylist"));
    Ok(Self {
      handle,
      sec_uid,
      can_create,
    })
  }

  /// The profile page, where the playlist calls are made from.
  pub fn page(&self) -> String {
    refs::user_url(&self.handle)
  }

  pub fn url(&self, name: &str, id: &str) -> String {
    format!("{WWW}/@{}/playlist/{}-{id}", self.handle, parse::slug(name))
  }

  /// Your playlists in the profile's order, every page of them.
  pub async fn playlists(&self, api: &Api) -> Result<Vec<Collection>> {
    let mut all: Vec<Collection> = Vec::new();
    let mut cursor = "0".to_owned();
    for _ in 0..LIST_PAGES {
      let params = [
        ("secUid", self.sec_uid.clone()),
        ("count", "20".to_owned()),
        ("cursor", cursor.clone()),
      ];
      let v = api.get(LIST, &params).await?;
      let found = ["playList", "mixList"]
        .iter()
        .flat_map(|k| v.list(k))
        .filter_map(|f| parse::folder(f, "playlist"));
      for c in found {
        if !all.iter().any(|a| a.id == c.id) {
          all.push(c);
        }
      }
      match videos::next_cursor(&v) {
        Some(next) if next != cursor => cursor = next,
        _ => return Ok(all),
      }
    }
    Err(Error::unsupported(format!(
      "more than {} playlists",
      LIST_PAGES * 20
    )))
  }

  /// One of your playlists by id, link or exact name, from your profile's
  /// list: what is not there is not yours.
  pub async fn playlist(&self, api: &Api, arg: &str) -> Result<Playlist> {
    let all = self.playlists(api).await?;
    let found = matching(&all, arg).ok_or_else(|| not_found(arg))?;
    Ok(Playlist {
      id: found.id.clone(),
      name: found.name.clone(),
      count: found.items.unwrap_or_default(),
      posts: Vec::new(),
    })
  }

  /// One of your posts that may join playlist `target`, held to what the
  /// page's post picker refuses: others' posts, posts that are not public,
  /// posts in another playlist (or in this one already).
  pub async fn post(&self, api: &Api, id: &str, target: Option<&str>) -> Result<Entry> {
    let post = videos::read(api, id).await?;
    let raw = post.raw.clone().unwrap_or_default();
    let refuse = |why: String| Err(Error::new(ErrorCode::PermissionDenied, why));
    let author = raw.str("author.uniqueId").unwrap_or_default();
    if !author.eq_ignore_ascii_case(&self.handle) {
      return refuse(format!(
        "post {id} is @{author}'s; a playlist takes your own posts"
      ));
    }
    let hidden = ["privateItem", "secret", "forFriend"]
      .iter()
      .any(|k| raw.bool(k) == Some(true));
    if hidden {
      return refuse(format!(
        "post {id} is not public (Only public posts can be added to playlists): make it \
         public in its privacy settings first"
      ));
    }
    let playlist = raw.str("playlistId").filter(|p| p != "0");
    match (playlist.as_deref(), target) {
      (Some(p), Some(t)) if p == t => Err(Error::input(format!(
        "post {id} is in this playlist already; `playlist-sort` moves it"
      ))),
      (Some(p), _) => refuse(format!(
        "post {id} is in playlist {p} (This post has been added to another playlist): a \
         post is in one playlist at most; take it out there first with \
         `playlist-remove {p} {id}`"
      )),
      (None, _) => Ok(Entry::of(&post)),
    }
  }
}

impl Playlist {
  /// Read the playlist's posts in its order, every page of them.
  pub async fn read_posts(&mut self, api: &Api) -> Result<()> {
    let mut req = PageReq::first(30);
    for _ in 0..POST_PAGES {
      let page = videos::playlist_videos(api, &self.id, &req).await?;
      for post in &page.items {
        if self.position(&post.id).is_none() {
          self.posts.push(Entry::of(post));
        }
      }
      match page.next_cursor {
        Some(next) if req.cursor.as_deref() != Some(next.as_str()) => {
          req = PageReq::at(next, 30);
        }
        _ => return Ok(()),
      }
    }
    Err(Error::unsupported(format!(
      "playlists of more than {} posts",
      POST_PAGES * 30
    )))
  }

  pub fn position(&self, id: &str) -> Option<usize> {
    self.posts.iter().position(|e| e.id == id)
  }

  /// The post at `at`, as a summary row.
  pub fn row(&self, at: usize) -> String {
    format!("post {}: {}", at + 1, self.posts[at].row())
  }

  /// The summary's head: the playlist and its post count.
  pub fn change(&self) -> Change {
    let mut change = Change::new(format!("TikTok playlist {} {}:", self.id, self.name));
    change.row("posts", &self.count.to_string());
    change
  }

  pub fn done(&self, action: &str, me: &Me, name: &str) -> Action {
    Action::done(action, &self.id)
      .with_id(&self.id)
      .with_url(me.url(name, &self.id))
  }
}

impl Entry {
  fn of(post: &Post) -> Self {
    let text = post
      .title
      .clone()
      .or_else(|| post.text.clone())
      .filter(|t| !t.trim().is_empty())
      .unwrap_or_else(|| "(no text)".into());
    Self {
      id: post.id.clone(),
      label: truncate(&one_line(&text), 30),
    }
  }

  pub fn row(&self) -> String {
    format!("{} {}", self.id, self.label)
  }
}

/// The playlist `arg` names among `all`: its id, link or exact name.
pub fn matching<'a>(all: &'a [Collection], arg: &str) -> Option<&'a Collection> {
  let arg = arg.trim();
  let id = refs::playlist_id(arg).ok();
  all
    .iter()
    .find(|c| id.as_deref() == Some(c.id.as_str()) || c.name.trim() == arg)
}

/// The posts given, as ids (`#N` after a list of posts too), each once.
pub async fn post_ids(api: &Api, posts: &[String]) -> Result<Vec<String>> {
  let mut ids: Vec<String> = Vec::with_capacity(posts.len());
  for arg in posts {
    let id = refs::post_ref(api, &api.ctx.post_ref(arg)?).await?.id;
    if ids.contains(&id) {
      return Err(Error::input(format!("post {id} is given twice")));
    }
    ids.push(id);
  }
  Ok(ids)
}

pub fn not_found(arg: &str) -> Error {
  Error::not_found(format!(
    "no playlist of yours is {arg}; give its id, its link, its exact name or #N after \
     `atp tiktok playlists`"
  ))
}
