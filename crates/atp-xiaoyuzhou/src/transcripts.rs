//! Transcripts (文字稿) of episodes, as the app shows them to a logged-in
//! account: the episode names its transcript's media id
//! (`/v1/episode/get`), `/v1/episode-transcript/get` hands out the file's
//! address, and the file holds the timed segments.

use atp_core::{Error, Result, Transcript, Value, ValueExt, json};

use crate::api::Api;
use crate::{episodes, parse, refs};

const TRANSCRIPT: &str = "/v1/episode-transcript/get";
/// The language of a transcript that does not name one: undetermined.
const UNKNOWN_LANG: &str = "und";

pub async fn transcript(api: &Api, arg: &str) -> Result<Transcript> {
  let eid = refs::episode(api, arg).await?;
  api.app().require_login()?;
  let episode = episodes::app_episode(api, &eid).await?;
  let media = episode
    .first_str(&["transcript.mediaId", "media.id", "transcriptMediaId"])
    .ok_or_else(|| no_transcript(&eid))?;
  let meta = api
    .app()
    .post(TRANSCRIPT, &json!({"eid": eid, "mediaId": media}))
    .await?;
  let meta = meta.at("data");
  let url = meta
    .first_str(&["transcriptUrl", "url"])
    .ok_or_else(|| no_transcript(&eid))?;
  let file = file(api, &url).await?;
  let cues = parse::cues(&file, episode.f64("duration"));
  if cues.is_empty() {
    return Err(Error::upstream(
      "the transcript file holds no text ATP can read",
    ));
  }
  let lang = [meta, &file]
    .iter()
    .find_map(|v| v.first_str(&["language", "lang", "languageCode"]))
    .unwrap_or_else(|| UNKNOWN_LANG.to_owned());
  Ok(Transcript { lang, cues })
}

fn no_transcript(eid: &str) -> Error {
  Error::not_found(format!("episode {eid} has no transcript"))
}

/// The transcript file, from Xiaoyuzhou's CDN (no session sent).
async fn file(api: &Api, url: &str) -> Result<Value> {
  if !url.starts_with("https://") {
    return Err(Error::upstream(format!(
      "the transcript is at an unexpected address: {url}"
    )));
  }
  api
    .ctx
    .http
    .get(url)
    .no_cookies()
    .header("accept", "*/*")
    .send()
    .await?
    .check()?
    .value()
}
