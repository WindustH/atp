//! 1Point3Acres commands beyond the shared set: the boards (版块) and the
//! threads of one board.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::boards;
use crate::threads::{self, Order};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// The sections, boards and sub-boards (版块), with their numbers and thread types
  #[command(after_long_help = EX_BOARDS)]
  Boards,
  /// The threads of a board, newest first by default
  #[command(after_long_help = EX_BOARD)]
  Board {
    /// Board: its number, a forum-<fid>-1.html link, its name (海外面经) or #N after `boards`
    board: String,
    /// The order, as the board's page offers it
    #[arg(short, long, value_enum, default_value_t = Order::New)]
    sort: Order,
    /// Only threads of one type (分类): its name (我司要招人) or number, as `boards` lists them
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    kind: Option<String>,
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_BOARDS: &str = "\
Examples:
  atp 1point3acres boards
  atp 1point3acres boards -f json | jq '.data.items[] | {id, name, description}'
  atp 1point3acres board '#12'

The list is kept a day. Sections group boards and hold no threads of their own.
More: atp guide 1point3acres boards";

const EX_BOARD: &str = "\
Examples:
  atp 1point3acres board 145 -n 50                 # 海外面经, newest first
  atp 1point3acres board 海外职位内推 --type 我司要招人
  atp 1point3acres board 98 --sort hot
  atp 1point3acres board https://www.1point3acres.com/bbs/forum-28-1.html --sort replied

Board names and their types are as `boards` lists them.
More: atp guide 1point3acres board";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Boards => Data::Collections(Page::last(boards::list(api).await?)),
    Command::Board {
      board,
      sort,
      kind,
      page,
    } => Data::Posts(board_threads(api, &board, sort, kind, page).await?),
  })
}

/// `board`: the board and its type resolved, then its threads.
async fn board_threads(
  api: &Api,
  board: &str,
  sort: Order,
  kind: Option<String>,
  page: PageArgs,
) -> Result<Page<Post>> {
  // `#N` picks from the last list; a bare number is a board's own.
  let board = if board.trim().starts_with('#') {
    api.ctx.collection_ref(board)?
  } else {
    board.to_owned()
  };
  let (fid, entry) = boards::find(api, &board).await?;
  let kind = match (kind, entry) {
    (Some(kind), Some(entry)) => Some(boards::thread_type(&entry, &kind)?),
    (Some(kind), None) => Some(kind.trim().parse().map_err(|_| {
      atp_core::Error::input(format!("--type takes the type's number for board {fid}"))
    })?),
    (None, _) => None,
  };
  page
    .collect_dated(async |req| threads::board(api, fid, sort, kind, &req).await)
    .await
}
