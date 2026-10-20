//! Groups (星球): the ones you joined (`/v2/groups`), one group's details
//! (`/v2/groups/<gid>`, for members) and the public preview of any group
//! (`pub-api.zsxq.com/v2/groups/<gid>`: the group and a few of its topics,
//! as its public page shows them, without an account).

use atp_core::{Collection, Error, ErrorCode, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::parse;

/// The groups you joined, in the order the app lists them.
pub async fn joined(api: &Api) -> Result<Vec<Collection>> {
  api.need_login()?;
  let v = api.get("/v2/groups", &[]).await?;
  let groups: Vec<Collection> = v.list("groups").iter().filter_map(parse::group).collect();
  api.keep_groups(groups.iter().map(|g| g.id.clone()).collect());
  Ok(groups)
}

/// The ids of the groups you joined, asked once a run.
pub async fn ids(api: &Api) -> Result<Vec<String>> {
  if let Some(ids) = api.known_groups() {
    return Ok(ids);
  }
  Ok(joined(api).await?.into_iter().map(|g| g.id).collect())
}

/// One group: what a member sees, or its public preview for anyone else.
pub async fn one(api: &Api, gid: &str) -> Result<Collection> {
  if api.logged_in() {
    match api.get(&format!("/v2/groups/{gid}"), &[]).await {
      Ok(v) => {
        if let Some(group) = parse::group(v.at("group")) {
          return Ok(group);
        }
      }
      // Not a member (any more): the public preview still says what it is.
      Err(e) if e.code == ErrorCode::PermissionDenied => {}
      Err(e) => return Err(e),
    }
  }
  Ok(preview(api, gid).await?.0)
}

/// The public preview of a group: the group and the topics its public
/// page shows (a few, digests mostly, their texts cut short).
pub async fn preview(api: &Api, gid: &str) -> Result<(Collection, Vec<Post>)> {
  let v = api.public(&format!("/v2/groups/{gid}")).await?;
  let mut group = parse::group(v.at("group"))
    .ok_or_else(|| Error::not_found(format!("知识星球 shows no public page for group {gid}")))?;
  group.extra.insert("preview".into(), json!(true));
  if let Some(url) = v.str("invite_url") {
    group.extra.insert("invite_url".into(), json!(url));
  }
  let topics = v
    .list("topics")
    .iter()
    .filter_map(|t| preview_topic(t, &v))
    .collect();
  Ok((group, topics))
}

/// A topic of a preview, with the group it is in.
fn preview_topic(topic: &Value, preview: &Value) -> Option<Post> {
  let mut post = parse::topic(topic)?;
  if let Some(name) = preview.str("group.name")
    && let Some(group) = post.extra.get_mut("group")
  {
    group["name"] = json!(name);
  }
  post.extra.insert("preview".into(), json!(true));
  Some(post)
}
