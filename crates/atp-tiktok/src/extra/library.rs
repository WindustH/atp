//! Your library beyond the shared commands: the sounds you saved, and
//! follow requests.

use atp_core::cli::PageArgs;
use atp_core::{Data, Result};

use crate::api::Api;
use crate::{relations, sounds};

const EX_SAVE_MUSIC: &str = "\
Examples:
  atp tiktok saved-music
  atp tiktok save-music https://www.tiktok.com/music/original-sound-7215829446516345602
  atp tiktok save-music '#2' '#3' --undo

More: atp guide tiktok save-music";

#[derive(Debug, clap::Subcommand)]
pub enum LibraryCommand {
  /// Sounds you saved (Favorites → Sounds)
  SavedMusic {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Save sounds, or unsave them with --undo
  #[command(after_long_help = EX_SAVE_MUSIC)]
  SaveMusic {
    /// Sounds: id, /music/ link or #N after `saved-music`
    #[arg(required = true, value_name = "SOUND")]
    sounds: Vec<String>,
    /// Unsave them
    #[arg(long)]
    undo: bool,
  },
  /// Approve requests to follow your private account
  ApproveFollower {
    /// Users: handle, @handle, profile link, secUid or #N
    #[arg(required = true, value_name = "USER")]
    users: Vec<String>,
  },
}

pub async fn run(api: &Api, command: LibraryCommand) -> Result<Data> {
  match command {
    LibraryCommand::SavedMusic { page } => Ok(Data::Collections(
      page.collect(async |r| sounds::saved(api, &r).await).await?,
    )),
    LibraryCommand::SaveMusic { sounds, undo } => {
      let sounds = sounds
        .iter()
        .map(|s| api.ctx.collection_ref(s))
        .collect::<Result<Vec<_>>>()?;
      Ok(Data::actions(sounds::save(api, &sounds, undo).await?))
    }
    LibraryCommand::ApproveFollower { users } => {
      let users = users
        .iter()
        .map(|u| api.ctx.user_ref(u))
        .collect::<Result<Vec<_>>>()?;
      Ok(Data::actions(relations::approve(api, &users).await?))
    }
  }
}
