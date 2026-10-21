//! Managing your playlists: creating, renaming and deleting them, their
//! posts and their order on your profile.

use atp_core::{Data, Result};

use crate::api::Api;
use crate::playlists;

#[derive(Debug, clap::Subcommand)]
pub enum PlaylistCommand {
  /// Create a playlist of your public posts
  #[command(after_long_help = EX_CREATE)]
  PlaylistCreate {
    /// Name, 25 characters at most
    title: String,
    /// Your public posts, in playlist order: id, link or #N (1 to 100)
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Rename one of your playlists
  #[command(after_long_help = EX_EDIT)]
  PlaylistEdit {
    /// Playlist: id, link, exact name or #N after `playlists`
    playlist: String,
    /// New name, 25 characters at most
    #[arg(long, required = true)]
    title: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Delete one of your playlists; its posts stay
  #[command(after_long_help = EX_DELETE)]
  PlaylistDelete {
    /// Playlist: id, link, exact name or #N after `playlists`
    playlist: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Add your public posts to one of your playlists
  #[command(after_long_help = EX_ADD)]
  PlaylistAdd {
    /// Playlist: id, link, exact name or #N after `playlists`
    playlist: String,
    /// Your public posts: id, link or #N
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Take posts out of one of your playlists; they stay on your account
  #[command(after_long_help = EX_REMOVE)]
  PlaylistRemove {
    /// Playlist: id, link, exact name or #N after `playlists`
    playlist: String,
    /// Posts in it: id, link or #N
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Reorder a playlist's posts: those given first, the others after
  #[command(after_long_help = EX_SORT)]
  PlaylistSort {
    /// Playlist: id, link, exact name or #N after `playlists`
    playlist: String,
    /// Posts in it, in their new order: id, link or #N
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Reorder your playlists on your profile: those given first, the others after
  #[command(after_long_help = EX_ORDER)]
  PlaylistsSort {
    /// Your playlists in their new order: id, link, exact name or #N after `playlists`
    #[arg(required = true, value_name = "PLAYLIST")]
    playlists: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

pub async fn run(api: &Api, command: PlaylistCommand) -> Result<Data> {
  use PlaylistCommand as P;
  let playlist = |arg: &str| api.ctx.collection_ref(arg);
  let action = match command {
    P::PlaylistCreate { title, videos, yes } => {
      playlists::create(api, &title, &videos, yes).await?
    }
    P::PlaylistEdit {
      playlist: arg,
      title,
      yes,
    } => playlists::rename(api, &playlist(&arg)?, &title, yes).await?,
    P::PlaylistDelete { playlist: arg, yes } => {
      playlists::delete(api, &playlist(&arg)?, yes).await?
    }
    P::PlaylistAdd {
      playlist: arg,
      videos,
      yes,
    } => playlists::add(api, &playlist(&arg)?, &videos, yes).await?,
    P::PlaylistRemove {
      playlist: arg,
      videos,
      yes,
    } => playlists::remove(api, &playlist(&arg)?, &videos, yes).await?,
    P::PlaylistSort {
      playlist: arg,
      videos,
      yes,
    } => playlists::sort(api, &playlist(&arg)?, &videos, yes).await?,
    P::PlaylistsSort {
      playlists: args,
      yes,
    } => {
      let args = args
        .iter()
        .map(|arg| playlist(arg))
        .collect::<Result<Vec<_>>>()?;
      playlists::order(api, &args, yes).await?
    }
  };
  Ok(Data::Action(action))
}

const EX_CREATE: &str = "\
Examples:
  atp tiktok playlist-create 'Rust basics' 7512345678901234567 7512345678901234568
  atp tiktok studio -n 20             # your posts; then give them by #N, in playlist order
  atp tiktok playlist-create 'Rust basics' '#3' '#2' '#1'

A post must be yours and public, and is in one playlist at most. The
write is made by TikTok's page in a browser tab.
More: atp guide tiktok playlist-create";

const EX_EDIT: &str = "\
Examples:
  atp tiktok playlists
  atp tiktok playlist-edit '#1' --title 'Rust basics, part 1'

More: atp guide tiktok playlist-edit";

const EX_DELETE: &str = "\
Examples:
  atp tiktok playlist-delete 'Rust basics'
  atp tiktok playlist-delete 7212345678901234567 -y

More: atp guide tiktok playlist-delete";

const EX_ADD: &str = "\
Examples:
  atp tiktok playlist-add 'Rust basics' 7512345678901234569
  atp tiktok playlist-add '#1' https://www.tiktok.com/@me/video/7512345678901234569

A post must be yours and public, and is in one playlist at most.
More: atp guide tiktok playlist-add";

const EX_REMOVE: &str = "\
Examples:
  atp tiktok playlist-remove 'Rust basics' 7512345678901234568

More: atp guide tiktok playlist-remove";

const EX_SORT: &str = "\
Examples:
  atp tiktok playlist 'https://www.tiktok.com/@me/playlist/Rust-basics-7212345678901234567'
  atp tiktok playlist-sort 'Rust basics' '#3'                 # the third post first
  atp tiktok playlist-sort 'Rust basics' 7512345678901234569 7512345678901234567

More: atp guide tiktok playlist-sort";

const EX_ORDER: &str = "\
Examples:
  atp tiktok playlists
  atp tiktok playlists-sort '#3'                        # the third playlist first
  atp tiktok playlists-sort 'Rust basics' 'Go basics'

More: atp guide tiktok playlists-sort";
