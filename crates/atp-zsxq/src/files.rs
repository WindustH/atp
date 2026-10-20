//! `download` of a topic: its pictures (links the API gives, signed for a
//! while) and its files. A file's link is asked when it is fetched
//! (`/v2/files/<id>/download_url`, as the app's download button asks; the
//! site counts it as a download), and the file is saved under its own
//! name through a `.part` file a later run continues. Videos are not
//! downloaded.

use std::path::{Path, PathBuf};

use atp_core::download::{self, Link};
use atp_core::{Error, FileEntry, Media, MediaKind, Page, Post, Result, ValueExt};

use crate::api::{API, Api, WEB};
use crate::topics;

/// The link of a file's download link, which stands for the file in the
/// media list until it is fetched.
fn file_link(id: &str) -> String {
  format!("{API}/v2/files/{id}/download_url")
}

fn file_id(url: &str) -> Option<&str> {
  url
    .strip_prefix(API)?
    .strip_prefix("/v2/files/")?
    .strip_suffix("/download_url")
}

/// The topic, and its pictures and files to save.
pub async fn media(api: &Api, arg: &str) -> Result<(Post, Vec<Media>)> {
  let post = topics::read(api, arg).await?;
  let mut media = post.media.clone();
  let files = post.extra.get("files").cloned().unwrap_or_default();
  for file in files.as_array().into_iter().flatten() {
    let Some(id) = file.str("id") else { continue };
    let mut item = Media::new(MediaKind::File, file_link(&id));
    item.alt = file.str("name");
    item.size = file.u64("size");
    media.push(item);
  }
  Ok((post, media))
}

/// Save one file of a topic under its own name; `None` for pictures,
/// which the shared downloader fetches.
pub async fn save(api: &Api, media: &Media, dir: &Path) -> Option<Result<PathBuf>> {
  let id = file_id(&media.url)?;
  let entry = FileEntry {
    id: id.to_owned(),
    name: media.alt.clone().unwrap_or_else(|| format!("file-{id}")),
    size: media.size,
    ..FileEntry::default()
  };
  Some(save_file(api, &entry, dir).await)
}

async fn save_file(api: &Api, entry: &FileEntry, dir: &Path) -> Result<PathBuf> {
  let saved = download::files(
    &api.ctx,
    entry,
    dir,
    async |_, _| Ok(Page::last(Vec::new())),
    async |file, _| link(api, &file.id).await,
  )
  .await?;
  saved
    .first()
    .map(|d| PathBuf::from(&d.path))
    .ok_or_else(|| Error::upstream(format!("知识星球 gave nothing for file {}", entry.id)))
}

async fn link(api: &Api, id: &str) -> Result<Link> {
  let v = api
    .get(&format!("/v2/files/{id}/download_url"), &[])
    .await?;
  let url = v
    .str("download_url")
    .ok_or_else(|| Error::upstream(format!("知识星球 gave no download link for file {id}")))?;
  Ok(Link {
    url,
    headers: vec![("referer".into(), format!("{WEB}/"))],
  })
}
