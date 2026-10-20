//! Users: the account of the login, profiles, the artists a user
//! follows, and a user's artworks and novels. A user's works come in two
//! steps, as the profile pages load them: the numbers of all their works
//! (`profile/all`, kept for the run), newest first, then a page of them
//! in full (`profile/illusts` or `profile/novels`, up to 48 at a time).

use atp_core::{Page, PageReq, Post, Result, User, Value, ValueExt};

use crate::api::{Api, Query};
use crate::parse;
use crate::refs;

/// The most works the site gives in one page of a profile.
const BATCH: usize = 48;
/// The most users of a following list a page asks for.
const FOLLOWING_PAGE: usize = 24;

pub async fn whoami(api: &Api) -> Result<User> {
  let uid = api.need_login()?;
  let counts = api.get("/ajax/user/extra", Vec::new()).await?;
  let mut user = profile(api, &uid).await?;
  user.stats.following = counts.count("following").or(user.stats.following);
  user.stats.followers = counts.count("followers");
  Ok(user)
}

async fn profile(api: &Api, uid: &str) -> Result<User> {
  let v = api
    .get(&format!("/ajax/user/{uid}"), vec![("full", "1".to_owned())])
    .await?;
  Ok(parse::user(&v))
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  profile(api, &refs::user(arg)?).await
}

/// The artists a user follows (public follows; yours include the private
/// ones only with `rest=hide`, which ATP does not ask for).
pub async fn following(api: &Api, arg: &str, page: &PageReq) -> Result<Page<User>> {
  let uid = refs::user(arg)?;
  let offset = page.number_or(0);
  let query: Query = vec![
    ("offset", offset.to_string()),
    ("limit", FOLLOWING_PAGE.to_string()),
    ("rest", "show".to_owned()),
    ("tag", String::new()),
    ("acceptingRequests", "0".to_owned()),
  ];
  let v = api
    .get(&format!("/ajax/user/{uid}/following"), query)
    .await?;
  let users: Vec<User> = v.list("users").iter().map(parse::followed).collect();
  let seen = offset + users.len() as u64;
  let next = (!users.is_empty() && seen < v.u64("total").unwrap_or(0)).then(|| seen.to_string());
  Ok(Page::new(users, next))
}

/// The numbers of the works of some kinds (`illusts`, `manga`,
/// `novels`) in a profile, newest first. A kind without works is an
/// empty list instead of an object.
fn ids(profile: &Value, kinds: &[&str]) -> Vec<u64> {
  let mut ids: Vec<u64> = kinds
    .iter()
    .filter_map(|kind| profile.at(kind).as_object())
    .flat_map(|works| works.keys().filter_map(|id| id.parse().ok()))
    .collect();
  ids.sort_unstable_by(|a, b| b.cmp(a));
  ids.dedup();
  ids
}

/// One page of a user's works of some kinds, from `profile/<what>`.
async fn works(
  api: &Api,
  arg: &str,
  kinds: &[&str],
  what: &str,
  page: &PageReq,
) -> Result<Page<Post>> {
  let uid = refs::user(arg)?;
  let all = ids(&api.profile(&uid).await?, kinds);
  let start = page.number_or(0) as usize;
  let size = page.size_within(BATCH);
  let batch: Vec<u64> = all.iter().skip(start).take(size).copied().collect();
  if batch.is_empty() {
    return Ok(Page::last(Vec::new()));
  }
  let mut query: Query = batch.iter().map(|id| ("ids[]", id.to_string())).collect();
  if what == "illusts" {
    query.push(("work_category", "illustManga".to_owned()));
  }
  query.push(("is_first_page", u8::from(start == 0).to_string()));
  let v = api
    .get(&format!("/ajax/user/{uid}/profile/{what}"), query)
    .await?;
  let found = v.at("works");
  let posts = batch
    .iter()
    .map(|id| found.at(&id.to_string()))
    .filter(|row| row.is_object())
    .filter(|row| api.logged_in() || !parse::restricted(row))
    .map(|row| match what {
      "novels" => parse::novel(row),
      _ => parse::artwork(row),
    })
    .collect();
  let end = start + batch.len();
  Ok(Page::new(posts, (end < all.len()).then(|| end.to_string())))
}

/// A user's illustrations, manga and ugoira, newest first.
pub async fn artworks(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  works(api, arg, &["illusts", "manga"], "illusts", page).await
}

/// A user's novels, newest first.
pub async fn novels(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  works(api, arg, &["novels"], "novels", page).await
}
