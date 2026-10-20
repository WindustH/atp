//! Hupu commands beyond the shared set: the boards (版块) by zone, the
//! threads of one board, and lighting (亮) a reply.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Result};

use crate::api::Api;
use crate::boards::{self, Order};
use crate::comments;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// The boards (版块) of every zone, as the front page lists them
  #[command(after_long_help = EX_BOARDS)]
  Boards,
  /// The threads of a board, latest replies first by default
  #[command(after_long_help = EX_BOARD)]
  Board {
    /// Board: its link (bbs.hupu.com/history), the part after the host (history, 502), its name (历史区) or #N after `boards`
    board: String,
    /// The order, as the board's tabs offer it
    #[arg(short, long, value_enum, default_value_t = Order::Replied)]
    sort: Order,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Light (亮) a reply, Hupu's like, or take the light back
  #[command(visible_alias = "light", after_long_help = EX_LIKE_COMMENT)]
  LikeComment {
    /// Thread: link, number or #N
    thread: String,
    /// Reply: the id `comments` prints
    reply: String,
    /// Take the light back instead
    #[arg(long)]
    undo: bool,
  },
}

const EX_BOARDS: &str = "\
Examples:
  atp hupu boards
  atp hupu boards -f json | jq '.data.items[] | {id, name, zone: .extra.zone}'
  atp hupu board '#3'

The list is kept a day.
More: atp guide hupu boards";

const EX_BOARD: &str = "\
Examples:
  atp hupu board history -n 100              # 历史区, latest replies first
  atp hupu board 步行街主干道 --sort new
  atp hupu board https://bbs.hupu.com/502 --sort hot

Board names are as `boards` lists them; any board's link works.
More: atp guide hupu board";

const EX_LIKE_COMMENT: &str = "\
Examples:
  atp hupu comments 643091945 --sort lights
  atp hupu like-comment 643091945 60346
  atp hupu light '#1' 60346 --undo

Needs a login. Threads themselves are not liked.
More: atp guide hupu like-comment";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Boards => Data::Collections(Page::last(boards::list(api).await?)),
    Command::Board { board, sort, page } => {
      let board = if board.trim().starts_with('#') {
        api.ctx.collection_ref(&board)?
      } else {
        board
      };
      let place = boards::find(api, &board).await?;
      Data::Posts(
        page
          .collect_dated(async |req| boards::threads(api, &place, sort, &req).await)
          .await?,
      )
    }
    Command::LikeComment {
      thread,
      reply,
      undo,
    } => {
      let thread = api.ctx.post_ref(&thread)?;
      Data::Action(comments::light(api, &thread, &reply, undo).await?)
    }
  })
}
