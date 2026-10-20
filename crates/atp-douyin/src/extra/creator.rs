//! The creator platform beyond publishing: your works with their numbers,
//! your drafts, and what the publish form offers a post (locations, hot
//! spots, official activities).

use atp_core::cli::PageArgs;
use atp_core::{Data, Result};

use crate::api::Api;
use crate::{drafts, insights, places, topics};

#[derive(Debug, clap::Subcommand)]
pub enum CreatorCommand {
  /// Your works with their creator numbers, review state and visibility
  #[command(after_long_help = EX_WORKS)]
  Works {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Your drafts (草稿) on the creator platform
  Drafts {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Find locations (POI, 位置) a post can be tagged with
  #[command(after_long_help = EX_POI)]
  Poi {
    /// Place name or address
    keyword: String,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Hot spots (热点) a post can be linked to
  #[command(after_long_help = EX_HOTSPOTS)]
  Hotspots {
    /// Words to look for; Douyin's recommendations when left out
    words: Option<String>,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Official activities (官方活动) open to your account
  Activities {
    #[command(flatten)]
    page: PageArgs,
  },
}

pub async fn run(api: &Api, command: CreatorCommand) -> Result<Data> {
  use CreatorCommand as C;
  Ok(match command {
    C::Works { page } => Data::Posts(
      page
        .collect_dated(async |r| insights::works(api, &r).await)
        .await?,
    ),
    C::Drafts { page } => Data::Posts(
      page
        .collect_dated(async |r| drafts::list(api, &r).await)
        .await?,
    ),
    C::Poi { keyword, page } => Data::Collections(
      page
        .collect(async |r| places::search(api, &keyword, &r).await)
        .await?,
    ),
    C::Hotspots { words, page } => Data::Posts(
      page
        .collect(async |_| topics::hotspots(api, words.as_deref()).await)
        .await?,
    ),
    C::Activities { page } => Data::Collections(
      page
        .collect(async |_| topics::activities(api).await)
        .await?,
    ),
  })
}

const EX_WORKS: &str = "\
Examples:
  atp douyin works -n 100 -f csv > works.csv
  atp douyin insights '#1'            # the numbers of one of them

Rows carry plays, likes, comments, shares and saves in `metrics`, the
other numbers (completion rate, 2 s bounce rate, cover click rate ...),
`review` and `visibility` in `extra`.
More: atp guide douyin works";

const EX_POI: &str = "\
Examples:
  atp douyin poi 三里屯
  atp douyin poi '上海 外滩' -n 30

More: atp guide douyin poi";

const EX_HOTSPOTS: &str = "\
Examples:
  atp douyin hotspots                 # recommended, then all
  atp douyin hotspots 春节

More: atp guide douyin hotspots";
