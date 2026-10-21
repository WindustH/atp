//! Hashtags (话题): the posts of one, as its page lists them
//! (`challenge/aweme`, 综合 order), and finding one by name through the
//! web search's hashtag results (`challenge/search`).

use atp_core::{Error, Page, PageReq, Post, Result, ValueExt};

use crate::api::Api;
use crate::{parse, refs};

const POSTS: &str = "/aweme/v1/web/challenge/aweme/";
const SEARCH: &str = "/aweme/v1/web/challenge/search/";
/// Hashtags looked through for one of the exact name.
const SEARCH_COUNT: u32 = 10;

/// The posts of a hashtag: an id, a douyin.com/hashtag/… link or a name.
pub async fn posts(api: &Api, arg: &str, page: &PageReq) -> Result<Page<Post>> {
  let id = match refs::hashtag(arg) {
    Some(id) => id,
    None => by_name(api, arg).await?,
  };
  let cursor = page.number_or(0);
  let v = api
    .get(POSTS)
    .arg("ch_id", &id)
    .arg("query_type", 0)
    .arg("sort_type", 0)
    .arg("offset", cursor)
    .arg("cursor", cursor)
    .arg("count", page.size_within(20))
    .send()
    .await?;
  Ok(parse::awemes(&v, "aweme_list", "cursor"))
}

/// The id of the hashtag named exactly so (a leading `#` is dropped).
async fn by_name(api: &Api, arg: &str) -> Result<String> {
  let name = arg.trim().trim_start_matches(['#', '＃']).trim();
  if name.is_empty() {
    return Err(Error::input(
      "give a hashtag: its id, a douyin.com/hashtag/… link or its name",
    ));
  }
  let v = api
    .get(SEARCH)
    .arg("keyword", name)
    .arg("count", SEARCH_COUNT)
    .arg("cursor", 0)
    .send()
    .await?;
  v.list("challenge_list")
    .iter()
    .map(|c| c.at("challenge_info"))
    .find(|c| {
      c.str("cha_name")
        .is_some_and(|n| n.to_lowercase() == name.to_lowercase())
    })
    .and_then(|c| c.str("cid"))
    .ok_or_else(|| {
      Error::not_found(format!("no Douyin hashtag #{name}"))
        .with_hint("`atp douyin search -t topic WORDS` lists hashtags with their ids")
    })
}
