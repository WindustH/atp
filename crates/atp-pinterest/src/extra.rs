//! Pinterest commands beyond the shared set: a board's pins, and creating
//! a pin from a picture on the web.

use atp_core::cli::PageArgs;
use atp_core::{Data, Result};

use crate::api::Api;
use crate::interactions::{self, NewPin};
use crate::pins;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// The pins of a board
  #[command(after_long_help = EX_BOARD)]
  Board {
    /// Board: its link, <username>/<board>, its number, or #N after `collections` or `search -t topic`
    board: String,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Create a pin on one of your boards from a picture's web address (needs a login)
  #[command(after_long_help = EX_CREATE_PIN)]
  CreatePin {
    /// The picture's address (https://…/picture.jpg), which Pinterest fetches itself
    image: String,
    /// Your board: its link, <username>/<board>, its number or #N
    #[arg(long)]
    board: String,
    /// Title of the pin
    #[arg(long)]
    title: Option<String>,
    /// Description of the pin
    #[arg(long, value_name = "TEXT")]
    description: Option<String>,
    /// Where the pin leads (a web page)
    #[arg(long, value_name = "URL")]
    link: Option<String>,
  },
}

const EX_BOARD: &str = "\
Examples:
  atp pinterest collections pinterest
  atp pinterest board '#1' -n 50
  atp pinterest board https://www.pinterest.com/pinterest/official-news/

More: atp guide pinterest board";

const EX_CREATE_PIN: &str = "\
Examples:
  atp pinterest create-pin https://example.com/photo.jpg --board me/recipes \\
    --title 'Plum cake' --link https://example.com/plum-cake

More: atp guide pinterest create-pin";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Board { board, page } => {
      let board = api.ctx.collection_ref(&board)?;
      Data::Posts(
        page
          .collect_dated(async |req| pins::on_board(api, &board, &req).await)
          .await?,
      )
    }
    Command::CreatePin {
      image,
      board,
      title,
      description,
      link,
    } => {
      let board = api.ctx.collection_ref(&board)?;
      let pin = NewPin {
        image: &image,
        board: &board,
        title: title.as_deref(),
        description: description.as_deref(),
        link: link.as_deref(),
      };
      Data::Action(interactions::create(api, &pin).await?)
    }
  })
}
