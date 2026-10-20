//! Locations (POI, 位置) a post can be tagged with, found as the creator
//! platform's publish form finds them (`life/video_api/search/poi`), and
//! the place `post --location` attaches, as the form's location box
//! picks it.

use std::time::Duration;

use atp_core::{Collection, Error, ErrorCode, Page, PageReq, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::creator::Creator;
use crate::items::Place;
use crate::parse;

const SEARCH: &str = "/aweme/v1/life/video_api/search/poi/";
/// The location box's page size.
const COUNT: u64 = 12;
/// The box's region tab: 0 local (around the browser's position, which
/// ATP does not have), 2 domestic (国内), 3 abroad.
const DOMESTIC: u64 = 2;
/// The location tab of the box (`poi_anchor_tab`).
const LOCATION_TAB: u64 = 2;
/// The places of recent searches, which `post --location` takes by id
/// (the search finds places by name, not by id).
const KEPT: &str = "places";
const KEPT_FOR: Duration = Duration::from_secs(7 * 86_400);
const KEPT_MAX: usize = 300;
/// `poi_backend_type` of housing (房产), whose promoted places the box
/// sends to a lead form instead of picking them.
const HOUSING: &str = "060701";

/// Locations in mainland China matching `keyword`. Pages count from 1;
/// the answer's `page` is the next one.
pub async fn search(api: &Api, keyword: &str, page: &PageReq) -> Result<Page<Collection>> {
  let creator = Creator::open(api).await?;
  let number = page.number_or(1);
  let v = search_page(&creator, keyword, number).await?;
  let items: Vec<Collection> = v.list("poi_list").iter().filter_map(parse::poi).collect();
  let more = v.bool("has_more") == Some(true) && !items.is_empty();
  let next = v.u64("page").filter(|p| *p > number).unwrap_or(number + 1);
  Ok(Page::new(items, more.then(|| next.to_string())))
}

/// One page of the box's search; its places are kept for `post
/// --location`.
async fn search_page(creator: &Creator<'_>, keyword: &str, number: u64) -> Result<Value> {
  let v = creator
    .get(SEARCH)
    .arg("count", COUNT)
    .arg("from_webapp", 1)
    .arg("keywords", keyword.trim())
    .arg("search_type", DOMESTIC)
    .arg("poi_anchor_tab", LOCATION_TAB)
    .arg("page", number)
    .send()
    .await?;
  keep(creator.api, &v);
  Ok(v)
}

/// The place `arg` names: the id of one a recent search found (`atp
/// douyin poi`), else one the search finds for `arg` under that id or
/// exactly that name.
pub async fn place(creator: &Creator<'_>, arg: &str) -> Result<Place> {
  let arg = arg.trim();
  let kept: Vec<Value> = creator
    .api
    .ctx
    .store
    .cache_get(KEPT, KEPT_FOR)
    .unwrap_or_default();
  if let Some(p) = kept
    .iter()
    .find(|p| p.str("poi_id").as_deref() == Some(arg))
  {
    return picked(p);
  }
  let v = search_page(creator, arg, 1).await?;
  let list = v.list("poi_list");
  let found = list
    .iter()
    .find(|p| p.str("poi_id").as_deref() == Some(arg))
    .or_else(|| {
      list
        .iter()
        .find(|p| p.str("poi_name").as_deref() == Some(arg))
    });
  match found {
    Some(p) => picked(&kept_fields(p, &v)),
    None => Err(Error::not_found(format!(
      "no Douyin place {arg}: give an id `atp douyin poi KEYWORD` printed (ATP knows the places of a week's searches) or a place's exact name"
    ))),
  }
}

/// A place as the box picks it from its list: the id, the name, and
/// `anchor_content`, which is the result's `anchor_post_ext.anchor_content`
/// with its `is_commerce_intention` set in (left out when the result has
/// none), as JSON. Places the box does not pick at once are refused.
fn picked(p: &Value) -> Result<Place> {
  let id = p
    .str("poi_id")
    .ok_or_else(|| Error::upstream("Douyin gave a place without an id"))?;
  let name = p
    .str("poi_name")
    .ok_or_else(|| Error::upstream(format!("Douyin gave place {id} no name")))?;
  let commerce = p.as_object().and_then(|o| o.get("is_commerce_intention"));
  if commerce == Some(&Value::Bool(true))
    && p.str("poi_backend_type.code").as_deref() == Some(HOUSING)
  {
    return Err(refused(format!(
      "{name} is a promoted housing place, which Douyin's location box sends to a lead form"
    )));
  }
  if p.bool("trigger_key_user_block") == Some(true) {
    return Err(refused(format!(
      "Douyin asks to confirm a notice before {name} can be attached"
    )));
  }
  let raw = p
    .str("anchor_post_ext.anchor_content")
    .unwrap_or_else(|| "{}".into());
  let mut content = match serde_json::from_str::<Value>(&raw) {
    Ok(Value::Object(map)) => map,
    _ => {
      return Err(Error::upstream(format!(
        "Douyin gave {name} an anchor_content that is not a JSON object: {raw}"
      )));
    }
  };
  match commerce {
    Some(value) => {
      content.insert("is_commerce_intention".into(), value.clone());
    }
    None => {
      content.shift_remove("is_commerce_intention");
    }
  }
  Ok(Place {
    id,
    name,
    content: Value::Object(content).to_string(),
  })
}

/// A place ATP does not attach, and where to attach it instead.
fn refused(why: String) -> Error {
  Error::new(
    ErrorCode::UnsupportedOperation,
    format!("{why}; attach it on creator.douyin.com"),
  )
}

/// Keep the places of a search answer, newest first, for a week.
fn keep(api: &Api, v: &Value) {
  let found: Vec<Value> = v
    .list("poi_list")
    .iter()
    .filter(|p| p.str("poi_id").is_some())
    .map(|p| kept_fields(p, v))
    .collect();
  if found.is_empty() {
    return;
  }
  let store = &api.ctx.store;
  let old: Vec<Value> = store.cache_get(KEPT, KEPT_FOR).unwrap_or_default();
  let ids: Vec<String> = found.iter().filter_map(|p| p.str("poi_id")).collect();
  let mut all = found;
  all.extend(
    old
      .into_iter()
      .filter(|p| !p.str("poi_id").is_some_and(|id| ids.contains(&id))),
  );
  all.truncate(KEPT_MAX);
  store.cache_put(KEPT, &all);
}

/// What [`picked`] reads of a search result. A place is held back for a
/// notice only when the answer carries one (`block_unqualified_key_user_notice`).
fn kept_fields(p: &Value, answer: &Value) -> Value {
  let mut kept = json!({
    "poi_id": p.str("poi_id"),
    "poi_name": p.str("poi_name"),
    "anchor_post_ext": { "anchor_content": p.str("anchor_post_ext.anchor_content") },
    "poi_backend_type": { "code": p.str("poi_backend_type.code") },
  });
  if let Some(commerce) = p.as_object().and_then(|o| o.get("is_commerce_intention")) {
    kept["is_commerce_intention"] = commerce.clone();
  }
  let notice = !answer.at("block_unqualified_key_user_notice").is_null();
  if notice && p.bool("trigger_key_user_block") == Some(true) {
    kept["trigger_key_user_block"] = json!(true);
  }
  kept
}
