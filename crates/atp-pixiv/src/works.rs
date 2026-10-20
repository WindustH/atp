//! Works: one artwork with every page, one novel with its text, what
//! `download` saves of them (an ugoira as the zip of its frames), and the
//! search of artworks and novels by keyword or tag.
//!
//! Works for adults (R-18, R-18G) are kept from guests: a guest's search
//! asks for all-ages works only and drops any other row, and a restricted
//! work is refused before its pages or files are asked for. With a login
//! the site applies the account's own setting.

use atp_core::text::encode_uri_component;
use atp_core::{
  Error, ErrorCode, Media, MediaKind, Page, PageReq, Post, Query, Result, Value, ValueExt,
};

use crate::api::Api;
use crate::parse;
use crate::refs::{self, WorkRef};

/// `search --sort`: newest first, oldest first, or the most popular
/// (overall, with men, with women), which the site sorts for Premium
/// accounts only.
pub const SORTS: &[&str] = &["new", "old", "popular", "popular-male", "popular-female"];
/// `search --filter`: one kind of work, or one age rating of artworks.
pub const FILTERS: &[&str] = &["illust", "manga", "ugoira", "novel", "safe", "r18"];

pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  match refs::work(arg)? {
    WorkRef::Artwork(id) => artwork(api, &id).await,
    WorkRef::Novel(id) => novel(api, &id).await,
  }
}

/// Refuse a work for adults to a guest.
pub fn guard(api: &Api, v: &Value, id: &str) -> Result<()> {
  if !parse::restricted(v) || api.logged_in() {
    return Ok(());
  }
  Err(
    Error::auth(format!(
      "pixiv work {id} is for adults only; ATP shows such works to a logged-in account whose settings allow them"
    ))
    .with_hint(api.ctx.login_hint()),
  )
}

/// An artwork's JSON (`/ajax/illust/<id>`), refused to guests when it is
/// for adults.
pub async fn artwork_json(api: &Api, id: &str) -> Result<Value> {
  let v = hinted(
    api,
    api.get(&format!("/ajax/illust/{id}"), Vec::new()).await,
  )?;
  guard(api, &v, id)?;
  Ok(v)
}

/// A work a guest cannot find may be one for adults, which the site keeps
/// from guests: say so.
fn hinted(api: &Api, answer: Result<Value>) -> Result<Value> {
  answer.map_err(|e| {
    if e.code != ErrorCode::NotFound || api.logged_in() || e.hint.is_some() {
      return e;
    }
    e.with_hint("works for adults show only to a login whose settings allow them")
  })
}

/// An artwork with every page in original size.
pub async fn artwork(api: &Api, id: &str) -> Result<Post> {
  let v = artwork_json(api, id).await?;
  let mut post = parse::artwork(&v);
  if v.count("pageCount").unwrap_or(1) > 1 {
    let pages = api
      .get(&format!("/ajax/illust/{id}/pages"), Vec::new())
      .await?;
    let media = parse::pages(pages.as_array().map(Vec::as_slice).unwrap_or_default());
    if !media.is_empty() {
      post.media = media;
    }
  }
  Ok(post)
}

/// A novel with its text, cover and pictures.
pub async fn novel(api: &Api, id: &str) -> Result<Post> {
  let v = hinted(api, api.get(&format!("/ajax/novel/{id}"), Vec::new()).await)?;
  guard(api, &v, id)?;
  Ok(parse::novel(&v))
}

/// What `download` saves: an artwork's pages in original size, an
/// ugoira's frames as the site's zip, a novel's cover and pictures.
pub async fn media(api: &Api, arg: &str) -> Result<(Post, Vec<Media>)> {
  let work = refs::work(arg)?;
  let post = match &work {
    WorkRef::Artwork(id) => artwork(api, id).await?,
    WorkRef::Novel(id) => novel(api, id).await?,
  };
  if post.kind != "ugoira" {
    let media = post.media.clone();
    return Ok((post, media));
  }
  let meta = api
    .get(
      &format!("/ajax/illust/{}/ugoira_meta", work.id()),
      Vec::new(),
    )
    .await?;
  let Some(zip) = meta.first_str(&["originalSrc", "src"]) else {
    let media = post.media.clone();
    return Ok((post, media));
  };
  Ok((post, vec![Media::new(MediaKind::File, zip)]))
}

/// The `order` of a `--sort` value.
fn order(sort: Option<&str>) -> &'static str {
  match sort {
    Some("old") => "date",
    Some("popular") => "popular_d",
    Some("popular-male") => "popular_male_d",
    Some("popular-female") => "popular_female_d",
    _ => "date_d",
  }
}

/// The `mode` of a search: all-ages works only for guests, R-18 ones only
/// on request with a login.
fn mode(api: &Api, filter: Option<&str>) -> Result<&'static str> {
  match filter {
    Some("r18") if !api.logged_in() => Err(
      Error::auth("searching works for adults needs a login whose settings allow them")
        .with_hint(api.ctx.login_hint()),
    ),
    Some("r18") => Ok("r18"),
    Some("safe") => Ok("safe"),
    _ if api.logged_in() => Ok("all"),
    _ => Ok("safe"),
  }
}

/// Works whose tags (or words) match, sixty a page.
pub async fn search(api: &Api, query: &Query, page: &PageReq) -> Result<Page<Post>> {
  let word = query.keyword.trim();
  if word.is_empty() {
    return Err(Error::input("search for something: a tag or some words"));
  }
  let filter = query.filter.as_deref();
  let (path, section, kind) = match filter {
    Some("illust") => ("illustrations", "illust", Some("illust_and_ugoira")),
    Some("ugoira") => ("illustrations", "illust", Some("ugoira")),
    Some("manga") => ("manga", "manga", Some("manga")),
    Some("novel") => ("novels", "novel", None),
    _ => ("artworks", "illustManga", Some("all")),
  };
  let no = page.number_or(1);
  let mut params = vec![
    ("word", word.to_owned()),
    ("order", order(query.sort.as_deref()).to_owned()),
    ("mode", mode(api, filter)?.to_owned()),
    ("p", no.to_string()),
    ("s_mode", "s_tag".to_owned()),
  ];
  if let Some(kind) = kind {
    params.push(("type", kind.to_owned()));
  }
  let path = format!("/ajax/search/{path}/{}", encode_uri_component(word));
  let v = api.get(&path, params).await?;
  let found = v.at(section);
  let rows = found.list("data");
  let posts = rows
    .iter()
    .filter(|row| row.str("id").is_some() && row.bool("isAdContainer") != Some(true))
    .filter(|row| api.logged_in() || !parse::restricted(row))
    .map(|row| match filter {
      Some("novel") => parse::novel(row),
      _ => parse::artwork(row),
    })
    .collect();
  let last = found.u64("lastPage").unwrap_or(no);
  let next = (no < last && !rows.is_empty()).then(|| (no + 1).to_string());
  Ok(Page::new(posts, next))
}
