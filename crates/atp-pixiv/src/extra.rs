//! Pixiv commands beyond the shared set: a user's novels.

use atp_core::cli::PageArgs;
use atp_core::{Data, Result};

use crate::api::Api;
use crate::users;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// A user's novels, newest first (`user-posts` lists their artworks)
  #[command(after_long_help = EX_NOVELS)]
  Novels {
    /// User: a pixiv.net/users/<id> link, the number, or #N
    user: String,
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_NOVELS: &str = "\
Examples:
  atp pixiv novels 11
  atp pixiv novels https://www.pixiv.net/users/11 -n 100
  atp pixiv read '#1'                # the first novel, with its text

More: atp guide pixiv novels";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Novels { user, page } => {
      let user = api.ctx.user_ref(&user)?;
      Data::Posts(
        page
          .collect_dated(async |req| users::novels(api, &user, &req).await)
          .await?,
      )
    }
  })
}
