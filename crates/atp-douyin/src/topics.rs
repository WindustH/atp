//! What a post can be put under on the creator platform, as its publish
//! form offers them: the hashtags it suggests for a word
//! (`challengesug`), hot spots (热点) to link a post to, and official
//! activities (官方活动), one of which `post --activity` joins.

use atp_core::text::from_unix;
use atp_core::{Collection, Error, ErrorCode, Page, Post, Query, Result, Value, ValueExt};
use jiff::Timestamp;

use crate::api::Api;
use crate::creator::Creator;
use crate::items::Tag;
use crate::parse;

const SUGGEST: &str = "/aweme/v1/search/challengesug/";
const HOTSPOTS: &str = "/aweme/v1/hotspot/recommend/";
const HOTSPOT_SEARCH: &str = "/aweme/v1/hotspot/search/";
const ACTIVITIES: &str = "/web/api/media/activity/get/";

/// The hashtags Douyin suggests for a word (`sug_list`), as the publish
/// form's hashtag box asks.
pub async fn suggestions(creator: &Creator<'_>, word: &str) -> Result<Vec<Value>> {
  let v = creator
    .get(SUGGEST)
    .arg("source", "challenge_create")
    .arg("aid", "2906")
    .arg("keyword", word)
    .send()
    .await?;
  Ok(v.list("sug_list").to_vec())
}

/// `search -t topic`: the hashtags suggested for the query.
pub async fn search(api: &Api, query: &Query) -> Result<Page<Collection>> {
  let creator = Creator::open(api).await?;
  let items = suggestions(&creator, query.keyword.trim())
    .await?
    .iter()
    .filter_map(parse::hashtag)
    .collect();
  Ok(Page::last(items))
}

/// Hot spots a post can be linked to (关联热点): with `words` those
/// matching them (`hotspot/search`), without them the ones Douyin
/// recommends (`extra.recommended`) and then all (`hotspot/recommend`
/// before a video is uploaded).
pub async fn hotspots(api: &Api, words: Option<&str>) -> Result<Page<Post>> {
  let creator = Creator::open(api).await?;
  let posts = match words.map(str::trim).filter(|w| !w.is_empty()) {
    Some(query) => {
      let v = creator
        .get(HOTSPOT_SEARCH)
        .arg("query", query)
        .arg("count", 50)
        .send()
        .await?;
      sentences(v.list("sentences"), false)
    }
    None => {
      let v = creator
        .get(HOTSPOTS)
        .arg("zip_uri", "")
        .arg("aid", 1128)
        .arg("aweme_name", "aweme")
        .send()
        .await?;
      let mut posts = sentences(v.list("rec_sentences"), true);
      for post in sentences(v.list("all_sentences"), false) {
        if !posts.iter().any(|p| p.id == post.id) {
          posts.push(post);
        }
      }
      posts
    }
  };
  Ok(Page::last(posts))
}

/// Hot spots as the hot boards' words, ranked in their list.
fn sentences(list: &[Value], recommended: bool) -> Vec<Post> {
  list
    .iter()
    .enumerate()
    .filter_map(|(i, s)| parse::hot_word(s, i + 1))
    .map(|mut post| {
      if recommended {
        post.extra.insert("recommended".into(), true.into());
      }
      post
    })
    .collect()
}

/// The official activities (官方活动) the publish form's activity picker
/// lists for the account.
pub async fn activities(api: &Api) -> Result<Page<Collection>> {
  let creator = Creator::open(api).await?;
  let v = activity_list(&creator).await?;
  let items = v
    .list("activity_list")
    .iter()
    .filter_map(parse::activity)
    .collect();
  Ok(Page::last(items))
}

async fn activity_list(creator: &Creator<'_>) -> Result<Value> {
  creator
    .get(ACTIVITIES)
    .arg("page", 1)
    .arg("size", 9999)
    .arg("need_challenge", 1)
    .send()
    .await
}

/// An official activity a post joins: its name, when it ends, and its
/// hashtags (`challenge` with `challenge_ids`), the first the one the
/// picker puts in.
#[derive(Debug, Clone)]
pub struct Activity {
  pub name: String,
  pub end: Option<Timestamp>,
  pub hashtags: Vec<Tag>,
}

/// The activity `id` (its `activity_id`) among those the picker lists:
/// unknown ids, ended activities and ones without a hashtag (which the
/// picker leaves out) are refused.
pub async fn activity(creator: &Creator<'_>, id: &str) -> Result<Activity> {
  joinable(&activity_list(creator).await?, id.trim())
}

/// [`activity`] in the list's answer.
fn joinable(v: &Value, id: &str) -> Result<Activity> {
  let a = v
    .list("activity_list")
    .iter()
    .find(|a| a.str("activity_id").as_deref() == Some(id))
    .ok_or_else(|| {
      Error::not_found(format!(
        "no current Douyin activity {id}; `atp douyin activities` lists the ones open to you"
      ))
    })?;
  let name = a
    .first_str(&["activity_name", "title"])
    .unwrap_or_else(|| id.to_owned());
  let end = a.i64("end_time").filter(|t| *t > 0).and_then(from_unix);
  if let Some(end) = end
    && end <= Timestamp::now()
  {
    return Err(Error::input(format!(
      "Douyin's activity {name} ended {}",
      atp_core::text::fmt_time(end)
    )));
  }
  let ids = a.list("challenge_ids");
  let hashtags: Vec<Tag> = a
    .list("challenge")
    .iter()
    .zip(ids)
    .filter_map(|(name, id)| {
      Some(Tag {
        name: name.str("")?,
        id: id.u64("").filter(|id| *id != 0)?,
      })
    })
    .collect();
  if hashtags.is_empty() {
    return Err(Error::new(
      ErrorCode::UnsupportedOperation,
      format!("Douyin's activity {name} has no hashtag to join it through"),
    ));
  }
  Ok(Activity {
    name,
    end,
    hashtags,
  })
}
