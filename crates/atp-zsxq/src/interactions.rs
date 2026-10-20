//! What a member does with a login, as the web app sends it: liking a
//! topic (`POST /v2/topics/<tid>/likes`, taken back with `DELETE`),
//! commenting (`POST /v2/topics/<tid>/comments`) and posting a topic to a
//! group (`POST /v2/groups/<gid>/topics`), each with a `{req_data}` body.
//! Text only: pictures and files are not uploaded. None of these has been
//! tried with a real account.

use atp_core::cli::confirm;
use atp_core::http::Method;
use atp_core::{Action, Draft, Error, Reply, Result, ValueExt, json};

use crate::api::Api;
use crate::parse::{group_url, topic_url};
use crate::{groups, refs};

pub async fn like(api: &Api, arg: &str, undo: bool) -> Result<Action> {
  let tid = refs::topic(arg)?;
  let path = format!("/v2/topics/{tid}/likes");
  if undo {
    api.send(Method::DELETE, &path, None).await?;
  } else {
    let body = json!({"req_data": {}});
    api.send(Method::POST, &path, Some(&body)).await?;
  }
  let action = if undo { "unlike" } else { "like" };
  Ok(Action::done(action, &tid).with_url(topic_url(&tid)))
}

/// Comment on a topic, or with `--reply-to` answer one of its comments.
pub async fn comment(api: &Api, arg: &str, reply: &Reply) -> Result<Action> {
  let tid = refs::topic(arg)?;
  let text = reply.text.trim();
  if text.is_empty() {
    return Err(Error::input("the comment is empty"));
  }
  if !reply.images.is_empty() {
    return Err(Error::input("知识星球 comments are sent as text only here"));
  }
  let mut req = json!({"text": text, "image_ids": [], "mentioned_user_ids": []});
  if let Some(to) = &reply.reply_to {
    let id = refs::comment(to)?;
    req["replied_comment_id"] = id.parse::<u64>().map(|n| json!(n)).unwrap_or(json!(id));
  }
  let body = json!({ "req_data": req });
  let path = format!("/v2/topics/{tid}/comments");
  let v = api.send(Method::POST, &path, Some(&body)).await?;
  let action = Action::done("comment", &tid).with_url(topic_url(&tid));
  Ok(match v.str("comment.comment_id") {
    Some(id) => action.with_id(id),
    None => action,
  })
}

/// Post a topic to the group `--topic` names, after asking; with
/// `--reply-to TOPIC` comment on that topic instead.
pub async fn publish(api: &Api, draft: &Draft) -> Result<Action> {
  check(draft)?;
  if let Some(topic) = &draft.reply_to {
    let reply = Reply {
      text: draft.text.clone(),
      ..Reply::default()
    };
    return comment(api, topic, &reply).await;
  }
  let [group] = draft.topics.as_slice() else {
    return Err(
      Error::input("name the group to post to with one --topic GROUP")
        .with_hint("`atp zsxq groups` lists yours"),
    );
  };
  let gid = refs::group(&api.ctx.collection_ref(group)?)?;
  api.need_login()?;
  let group = groups::one(api, &gid).await?;
  confirm(
    draft.yes,
    &format!("post this topic to the group {} ({gid})", group.name),
  )?;
  let body = json!({"req_data": {
    "type": "topic",
    "text": draft.text.trim(),
    "image_ids": [],
    "file_ids": [],
    "mentioned_user_ids": [],
  }});
  let path = format!("/v2/groups/{gid}/topics");
  let v = api.send(Method::POST, &path, Some(&body)).await?;
  let action = Action::done("publish", &gid);
  Ok(match v.str("topic.topic_id") {
    Some(id) => action.with_url(topic_url(&id)).with_id(id),
    None => action.with_url(group_url(&gid)),
  })
}

/// What a topic of this crate can carry: text only, no title, no quote.
fn check(draft: &Draft) -> Result<()> {
  if draft.text.trim().is_empty() {
    return Err(Error::input("the topic is empty"));
  }
  if draft.title.is_some() {
    return Err(Error::input(
      "知识星球 topics have no title here; start the text with it",
    ));
  }
  if !draft.images.is_empty() {
    return Err(Error::input("pictures are not uploaded; post text only"));
  }
  if draft.quote.is_some() {
    return Err(Error::unsupported("post --quote"));
  }
  Ok(())
}
