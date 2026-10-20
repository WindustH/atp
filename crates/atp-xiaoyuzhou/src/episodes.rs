//! Episodes: the episode page (show notes, the audio file, counts), its
//! hot comments, the file `download` saves, and the episode as the app's
//! API has it.

use atp_core::paging::slice;
use atp_core::{Comment, Error, ErrorCode, Media, Page, PageReq, Post, Result, Value, ValueExt};

use crate::api::Api;
use crate::refs::{self, Item};
use crate::{parse, shows};

const APP_EPISODE: &str = "/v1/episode/get";

/// The `pageProps` of `/episode/<id>`: `episode` and `comments`.
async fn data(api: &Api, id: &str) -> Result<Value> {
  let props = api.page(&format!("/episode/{id}")).await?;
  if props.str("episode.eid").is_none() {
    return Err(Error::not_found(format!("Xiaoyuzhou has no episode {id}")));
  }
  Ok(props)
}

pub async fn read(api: &Api, id: &str) -> Result<Post> {
  let props = data(api, id).await?;
  Ok(parse::episode(props.at("episode")))
}

/// A show or an episode; a bare id is tried as a show, then as an episode.
pub async fn read_any(api: &Api, arg: &str) -> Result<Post> {
  match refs::resolve(api, arg).await? {
    Item::Show(id) => shows::read(api, &id).await,
    Item::Episode(id) => read(api, &id).await,
    Item::User(_) => Err(Error::input("that is a user; use `user` or `user-posts`")),
    Item::Bare(id) => match shows::read(api, &id).await {
      Err(e) if e.code == ErrorCode::NotFound => read(api, &id).await,
      other => other,
    },
  }
}

/// The episode as the app's API has it (`data` of `/v1/episode/get`),
/// for a logged-in account: with its transcript's media id, and the audio
/// of paid episodes the account may play.
pub async fn app_episode(api: &Api, id: &str) -> Result<Value> {
  let v = api
    .app()
    .get(APP_EPISODE, &[("eid", id.to_owned())])
    .await?;
  let episode = v.at("data");
  if episode.str("eid").is_none() {
    return Err(Error::not_found(format!("Xiaoyuzhou has no episode {id}")));
  }
  Ok(episode.clone())
}

/// The hot comments the episode page shows (about 20, with some replies).
pub async fn comments(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Comment>> {
  let id = refs::episode(api, arg).await?;
  let props = data(api, &id).await?;
  let items: Vec<Comment> = props.list("comments").iter().map(parse::comment).collect();
  Ok(slice(items, page))
}

/// An episode and its audio file, for `download`. A paid episode's page
/// gives none; logged in, the app's API is asked for it.
pub async fn media(api: &Api, arg: &str) -> Result<(Post, Vec<Media>)> {
  let id = refs::episode(api, arg).await?;
  let mut post = read(api, &id).await?;
  if post.media.is_empty() && api.app().logged_in() {
    post.media = parse::episode(&app_episode(api, &id).await?).media;
  }
  if post.media.is_empty() {
    let paid = post.extra.contains_key("pay_type");
    let why = if paid {
      "this is a paid episode; only Xiaoyuzhou's app plays it"
    } else {
      "the episode page gives no audio file"
    };
    return Err(Error::new(ErrorCode::PermissionDenied, why));
  }
  let media = post.media.clone();
  Ok((post, media))
}
