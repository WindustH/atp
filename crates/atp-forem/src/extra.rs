//! Forem commands beyond the shared set: a tag's articles, the tags,
//! your drafts.

use atp_core::cli::PageArgs;
use atp_core::{Data, Result};
use clap::builder::PossibleValuesParser;

use crate::api::Api;
use crate::articles;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Articles with a tag: the popular ones, or the most liked of a period
  #[command(after_long_help = EX_TAG)]
  Tag {
    /// Tag: a name (`rust`), `#rust` or a /t/ link
    tag: String,
    /// The most liked of this period instead
    #[arg(long, value_name = "PERIOD", value_parser = PossibleValuesParser::new(articles::PERIODS))]
    top: Option<String>,
    #[command(flatten)]
    page: PageArgs,
  },
  /// The community's tags, the most used first
  Tags {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Your unpublished articles (needs the API key)
  Drafts {
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_TAG: &str = "\
Examples:
  atp forem tag rust
  atp forem tag '#webdev' --top week -n 10
  FOREM_URL=community.codenewbie.org atp forem tag beginners

More: atp guide forem tag";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Tag { tag, top, page } => {
      let tag = api.ctx.collection_ref(&tag)?;
      Data::Posts(
        page
          .collect_dated(async |req| articles::tagged(api, &tag, top.as_deref(), &req).await)
          .await?,
      )
    }
    Command::Tags { page } => Data::Collections(
      page
        .collect(async |req| articles::tags(api, &req).await)
        .await?,
    ),
    Command::Drafts { page } => Data::Posts(
      page
        .collect(async |req| articles::drafts(api, &req).await)
        .await?,
    ),
  })
}
