//! Live rooms: `/api-live/user/room/` turns a host's handle into the room,
//! whether it is live, its title, viewers and stream addresses (`live`);
//! `listen` takes a room id as it is.

use atp_core::text::from_secs;
use atp_core::{Error, Extra, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::parse;
use crate::refs::{self, LiveRef};

const USER_ROOM: &str = "/api-live/user/room/";
/// `liveRoom.status` of a room that is on air.
const LIVE: i64 = 2;

pub struct Room {
  pub id: String,
  /// How the room is named in chat ids: `@handle`, or the room id.
  pub key: String,
  pub title: String,
}

pub async fn find(api: &Api, arg: &str) -> Result<Room> {
  match refs::live(arg)? {
    LiveRef::Room(id) => Ok(Room {
      key: id.clone(),
      id,
      title: String::new(),
    }),
    LiveRef::Handle(handle) => by_handle(api, &handle).await,
  }
}

async fn by_handle(api: &Api, handle: &str) -> Result<Room> {
  let v = user_room(api, handle).await?;
  let room = v.at("data.liveRoom");
  match room_id(&v) {
    Some(id) if room.i64("status") == Some(LIVE) => Ok(Room {
      id,
      key: format!("@{handle}"),
      title: room.str("title").unwrap_or_default(),
    }),
    _ => Err(Error::not_found(format!("@{handle} is not live now"))),
  }
}

/// The host and the room of a handle, as the live page loads them; sent
/// without a signature, which TikTok answers.
async fn user_room(api: &Api, handle: &str) -> Result<Value> {
  let params = [
    ("aid", "1988"),
    ("app_language", "en"),
    ("app_name", "tiktok_web"),
    ("device_platform", "web_pc"),
    ("sourceType", "54"),
    ("uniqueId", handle),
  ];
  api
    .unsigned(USER_ROOM, &params, &refs::live_url(handle))
    .await
}

fn room_id(v: &Value) -> Option<String> {
  v.str("data.user.roomId")
    .filter(|id| !id.is_empty() && id != "0")
}

/// `live @HANDLE`: the host's room with its title, host, viewers and
/// stream addresses; a host who is not live comes with `extra.live` false
/// and the title, start and room of the last live.
pub async fn room(api: &Api, arg: &str) -> Result<Post> {
  let LiveRef::Handle(handle) = refs::live(arg)? else {
    return Err(Error::input(
      "`live` takes the host: @handle or a tiktok.com/@handle/live link (a room id alone names no host)",
    ));
  };
  let v = user_room(api, &handle).await?;
  let host = v.at("data.user");
  if host.str("uniqueId").is_none() {
    return Err(Error::not_found(format!("no TikTok user @{handle}")));
  }
  let room = v.at("data.liveRoom");
  let id = room_id(&v);
  Ok(Post {
    id: id.clone().unwrap_or_else(|| format!("@{handle}")),
    kind: "live".into(),
    title: room.str("title").filter(|t| !t.is_empty()),
    url: Some(refs::live_url(&handle)),
    author: Some(parse::user_brief(host)),
    created_at: room.i64("startTime").filter(|t| *t > 0).and_then(from_secs),
    extra: room_extra(room, id),
    raw: Some(v.clone()),
    ..Post::default()
  })
}

/// Live state, the room id, viewers in all; while live, viewers now and
/// the stream addresses by quality (those of an ended live play nothing).
fn room_extra(room: &Value, id: Option<String>) -> Extra {
  let mut extra = Extra::new();
  let live = room.i64("status") == Some(LIVE);
  extra.insert("live".into(), live.into());
  if let Some(id) = id {
    extra.insert("room_id".into(), id.into());
  }
  if let Some(n) = room.count("liveRoomStats.enterCount") {
    extra.insert("viewers_total".into(), n.into());
  }
  if !live {
    return extra;
  }
  if let Some(n) = room.count("liveRoomStats.userCount") {
    extra.insert("viewers".into(), n.into());
  }
  let streams = streams(room);
  if !streams.is_empty() {
    extra.insert("streams".into(), Value::Object(streams));
  }
  extra
}

/// The FLV and HLS addresses of each quality (`origin`, `uhd`, `hd`, `sd`
/// ...), from the JSON text in `streamData.pull_data.stream_data`.
fn streams(room: &Value) -> serde_json::Map<String, Value> {
  let Some(text) = room.str("streamData.pull_data.stream_data") else {
    return serde_json::Map::new();
  };
  let Ok(data) = serde_json::from_str::<Value>(&text) else {
    return serde_json::Map::new();
  };
  let Some(qualities) = data.at("data").as_object() else {
    return serde_json::Map::new();
  };
  qualities
    .iter()
    .filter_map(|(name, q)| {
      let main = q.at("main");
      let (flv, hls) = (main.str("flv"), main.str("hls"));
      (flv.is_some() || hls.is_some()).then(|| (name.clone(), json!({ "flv": flv, "hls": hls })))
    })
    .collect()
}
