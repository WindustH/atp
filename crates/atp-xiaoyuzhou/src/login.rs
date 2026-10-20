//! Logins to the app's API, as the iOS app does them.
//!
//! - Texted code: `POST /v1/auth/sendCode` with the number asks for a code
//!   by SMS; `POST /v1/auth/loginOrSignUpWithSMS` with the code answers
//!   with the token pair (`x-jike-access-token`, `x-jike-refresh-token`).
//!   As its name says, a number without an account gets one.
//! - Tokens: `login --token` takes the pair as copied from the app's
//!   traffic, or a refresh token alone, traded at once for a fresh pair.

use std::io::IsTerminal;

use atp_core::cli::prompt;
use atp_core::http::Method;
use atp_core::oauth::{ACCESS_TOKEN, EXPIRES_AT, REFRESH_TOKEN, Tokens};
use atp_core::{Error, Result, User, credentials, json};

use crate::api::Api;
use crate::app::{self, App};
use crate::users;

const SEND_CODE: &str = "/v1/auth/sendCode";
const SMS_LOGIN: &str = "/v1/auth/loginOrSignUpWithSMS";

/// `login --token`: `ACCESS_TOKEN|REFRESH_TOKEN`, or `REFRESH_TOKEN` alone.
pub async fn token(api: &Api, token: &str) -> Result<()> {
  let fields = credentials::expect(token, 1, 2, "ACCESS_TOKEN|REFRESH_TOKEN")?;
  let tokens = match fields.as_slice() {
    [refresh_token] => app::refresh(&api.app(), refresh_token).await?,
    [access, refresh_token] => Tokens {
      access: access.clone(),
      refresh: Some(refresh_token.clone()),
      expires_at: None,
    },
    _ => unreachable!("expect checked the count"),
  };
  keep(&api.app(), &tokens);
  Ok(())
}

/// `login-sms`: the code Xiaoyuzhou texts to `phone` (asked when left
/// out), typed in on the terminal.
pub async fn sms(api: &Api, phone: Option<String>) -> Result<User> {
  if !std::io::stdin().is_terminal() {
    return Err(Error::input(
      "`login-sms` asks for the texted code on a terminal",
    ));
  }
  let phone = match phone {
    Some(phone) => phone,
    None => prompt::line("Phone number (mainland China, or +CC NUMBER): ")?,
  };
  let (area, number) = mobile(&phone)?;
  let app = api.app();
  let number_body = json!({"mobilePhoneNumber": number, "areaCode": area});
  let sent = app
    .request(Method::POST, SEND_CODE, None)
    .json(&number_body)
    .send()
    .await?;
  app::check(sent)?;
  let code = prompt::line("Code texted to that number: ")?;
  if code.is_empty() || !code.bytes().all(|b| b.is_ascii_digit()) {
    return Err(Error::input("the code is the digits Xiaoyuzhou texted"));
  }
  let body = json!({"areaCode": area, "verifyCode": code, "mobilePhoneNumber": number});
  let resp = app
    .request(Method::POST, SMS_LOGIN, None)
    .json(&body)
    .send()
    .await?;
  let tokens = app::token_answer(resp)?
    .ok_or_else(|| Error::auth("Xiaoyuzhou took the code but handed out no tokens"))?;
  // Saved at once: the account is logged in even if reading it fails.
  app.save(&tokens)?;
  users::whoami(api).await
}

/// `13800138000` (mainland), or `+CC NUMBER`.
fn mobile(phone: &str) -> Result<(String, String)> {
  let phone = phone.trim();
  let (area, number) = match phone.strip_prefix('+') {
    Some(rest) => {
      let (cc, number) = rest
        .split_once(char::is_whitespace)
        .ok_or_else(|| Error::input("write a number abroad as `+CC NUMBER`"))?;
      (format!("+{cc}"), number.trim().replace([' ', '-'], ""))
    }
    None => ("+86".to_owned(), phone.replace([' ', '-'], "")),
  };
  let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
  if !digits(&area[1..]) || !digits(&number) {
    return Err(Error::input(format!("not a phone number: {phone}")));
  }
  Ok((area, number))
}

/// Keep the tokens in the session the login saves.
fn keep(app: &App<'_>, tokens: &Tokens) {
  let refresh_token = tokens.refresh.clone().unwrap_or_default();
  credentials::save(
    app.ctx,
    &[
      (ACCESS_TOKEN, tokens.access.as_str()),
      (REFRESH_TOKEN, refresh_token.as_str()),
      (EXPIRES_AT, ""),
    ],
  );
}
