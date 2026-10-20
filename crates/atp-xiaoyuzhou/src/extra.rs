//! Xiaoyuzhou commands beyond the shared set: the texted-code login and
//! episode transcripts.

use atp_core::{Data, Result};

use crate::api::Api;
use crate::{login, transcripts};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Log in with a phone number and the code Xiaoyuzhou texts to it
  #[command(after_long_help = EX_LOGIN_SMS)]
  LoginSms {
    /// Mainland number, or `+CC NUMBER` (asked when left out)
    phone: Option<String>,
  },
  /// Transcript (文字稿) of an episode, as timed lines (needs a login)
  #[command(visible_alias = "subtitle", after_long_help = EX_TRANSCRIPT)]
  Transcript {
    /// Episode: /episode/ link, `episode:<id>`, 24-digit id or #N
    episode: String,
  },
}

const EX_LOGIN_SMS: &str = "\
Examples:
  atp xiaoyuzhou login-sms                # asks for the number, then the code
  atp xiaoyuzhou login-sms 13800138000
  atp xiaoyuzhou login-sms '+852 51234567'

The code is typed in on the terminal. A number without an account gets
one, as in the app.
More: atp guide xiaoyuzhou login";

const EX_TRANSCRIPT: &str = "\
Examples:
  atp xiaoyuzhou transcript https://www.xiaoyuzhoufm.com/episode/6ad1c92a0e4e410a05bdc907
  atp xiaoyuzhou transcript '#1' -f jsonl > lines.jsonl

More: atp guide xiaoyuzhou transcript";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::LoginSms { phone } => Data::User(Box::new(login::sms(api, phone).await?)),
    Command::Transcript { episode } => {
      let episode = api.ctx.post_ref(&episode)?;
      Data::Transcript(transcripts::transcript(api, &episode).await?)
    }
  })
}
