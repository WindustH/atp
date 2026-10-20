//! Actions of the login, sent as the site's pages send them: saving a
//! pin to one of your boards or your profile (`RepinResource`), and
//! creating a pin from a picture on the web (`PinResource`). Removing a
//! save is not offered: it deletes the saved copy, a pin of your own.

use atp_core::{Action, Error, ErrorCode, Result, ValueExt, json};

use crate::api::Api;
use crate::{boards, parse, pins};

/// `favorite PIN [--folder BOARD]`: save a pin to a board, or without one
/// to your profile.
pub async fn save(api: &Api, arg: &str, board: Option<&str>, undo: bool) -> Result<Action> {
  if undo {
    return Err(Error::new(
      ErrorCode::UnsupportedOperation,
      "a save is a pin of your own on Pinterest; ATP does not delete pins",
    ));
  }
  api.need_login()?;
  let id = pins::id(api, arg).await?;
  let mut options = json!({ "pin_id": id });
  let mut source = "/".to_owned();
  if let Some(board) = board {
    let found = boards::find(api, board).await?;
    options["board_id"] = json!(found.str("id"));
    source = found.str("url").unwrap_or(source);
  }
  let data = api.act("RepinResource", "create", options, &source).await?;
  let mut action = Action::done("favorite", parse::pin_url(&id));
  if let Some(saved) = data.str("id") {
    action = action
      .with_id(saved.clone())
      .with_url(parse::pin_url(&saved));
  }
  Ok(action)
}

/// What `create-pin` takes.
pub struct NewPin<'a> {
  pub image: &'a str,
  pub board: &'a str,
  pub title: Option<&'a str>,
  pub description: Option<&'a str>,
  pub link: Option<&'a str>,
}

/// Create a pin on one of your boards from a picture's address.
pub async fn create(api: &Api, pin: &NewPin<'_>) -> Result<Action> {
  api.need_login()?;
  let image = url::Url::parse(pin.image.trim())
    .ok()
    .filter(|u| matches!(u.scheme(), "http" | "https"))
    .ok_or_else(|| Error::input(format!("not a picture's web address: {}", pin.image)))?;
  let board = boards::find(api, pin.board).await?;
  let source = board.str("url").unwrap_or_else(|| "/".to_owned());
  let mut options = json!({
    "board_id": board.str("id"),
    "image_url": image.as_str(),
    "method": "scraped",
  });
  let fields = [
    ("title", pin.title),
    ("description", pin.description),
    ("link", pin.link),
  ];
  for (key, value) in fields {
    if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
      options[key] = json!(value);
    }
  }
  let data = api.act("PinResource", "create", options, &source).await?;
  let id = data
    .str("id")
    .ok_or_else(|| Error::upstream("Pinterest created no pin (no id in its answer)"))?;
  Ok(
    Action::done("create-pin", pin.board)
      .with_id(id.clone())
      .with_url(parse::pin_url(&id)),
  )
}
