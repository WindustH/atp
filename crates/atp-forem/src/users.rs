//! Users and the login: profiles by username or id, your own account
//! with the key, and keeping the key (and the community it belongs to).

use atp_core::{Error, Result, User, ValueExt, credentials};

use crate::api::{self, Api, INSTANCE_KEY, KEY_KEY};
use crate::parse;
use crate::refs::{self, UserRef};

/// `API_KEY` for the community in use, or `INSTANCE|API_KEY`.
pub fn login(api: &Api, token: &str) -> Result<()> {
  let fields: Vec<&str> = token.trim().splitn(2, '|').map(str::trim).collect();
  let (instance, key) = match fields.as_slice() {
    [instance, key] => (api::instance_url(instance), *key),
    [key] => (api.instance(), *key),
    _ => (String::new(), ""),
  };
  if key.is_empty() || key.contains(char::is_whitespace) {
    return Err(Error::input(
      "expected `login --token API_KEY` or `login --token 'INSTANCE|API_KEY'`",
    ));
  }
  credentials::save(&api.ctx, &[(INSTANCE_KEY, &instance), (KEY_KEY, key)]);
  Ok(())
}

pub async fn whoami(api: &Api) -> Result<User> {
  api.require()?;
  let v = api.get("users/me", Vec::new()).await?;
  Ok(parse::user(&v, &api.instance()))
}

pub async fn user(api: &Api, arg: &str) -> Result<User> {
  let v = match refs::user(arg)? {
    UserRef::Id(id) => api.get(&format!("users/{id}"), Vec::new()).await?,
    UserRef::Name(name) => api.get("users/by_username", vec![("url", name)]).await?,
  };
  Ok(parse::user(&v, &api.instance()))
}

/// The username of a user reference (ids are looked up).
pub async fn username(api: &Api, user: &UserRef) -> Result<String> {
  match user {
    UserRef::Name(name) => Ok(name.clone()),
    UserRef::Id(id) => {
      let v = api.get(&format!("users/{id}"), Vec::new()).await?;
      v.str("username")
        .ok_or_else(|| Error::not_found(format!("no user {id} on {}", api.host())))
    }
  }
}
