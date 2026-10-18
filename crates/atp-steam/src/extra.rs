//! Steam commands beyond the shared set: reviews with every filter,
//! prices in several storefronts, and the games a profile owns.

use atp_core::cli::PageArgs;
use atp_core::paging::slice;
use atp_core::{Data, Result};
use clap::builder::PossibleValuesParser;

use crate::api::Api;
use crate::reviews::{self, Filter};
use crate::{apps, users};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// An app's reviews: order, positive or negative, language, where the game was bought
  #[command(after_long_help = EX_REVIEWS)]
  Reviews(ReviewArgs),
  /// Apps' prices in one or more storefronts
  #[command(after_long_help = EX_PRICES)]
  Prices {
    /// Apps: ids, store links, or #N from the last list
    #[arg(required = true, value_name = "APP")]
    apps: Vec<String>,
    /// Storefront countries, two letters each (`us,cn,tr`); STEAM_CC (or us) by default
    #[arg(long, value_delimiter = ',', value_name = "CC")]
    cc: Vec<String>,
  },
  /// Games a profile owns, with hours played (needs a Web API key)
  #[command(after_long_help = EX_GAMES)]
  Games {
    /// Profile: a SteamID64, a profile link, a custom name or #N; yours by default
    user: Option<String>,
    /// Only the games played in the last two weeks
    #[arg(long)]
    recent: bool,
    /// Order: most hours, last played, or by name
    #[arg(long, default_value = "playtime", value_parser = PossibleValuesParser::new(users::GAME_SORTS))]
    sort: String,
    #[command(flatten)]
    page: PageArgs,
  },
}

#[derive(Debug, clap::Args)]
pub struct ReviewArgs {
  /// App: an id, a store link, or #N from the last list
  app: String,
  /// Order: Steam's most helpful first, newest first, or most recently changed first
  #[arg(short, long, default_value = "helpful", value_parser = ["helpful", "recent", "updated"])]
  sort: String,
  /// Only positive or only negative reviews
  #[arg(long = "type", default_value = "all", value_parser = ["all", "positive", "negative"])]
  kind: String,
  /// Steam's name of a language (english, schinese, tchinese, japanese, koreana, german, french, russian, spanish …), or all
  #[arg(long, default_value = "all")]
  language: String,
  /// Where the game was bought: anywhere, on Steam, or elsewhere (keys)
  #[arg(long, default_value = "all", value_parser = ["all", "steam", "other"])]
  purchase: String,
  /// Helpful reviews of the last N days only (1 to 365)
  #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=365))]
  days: Option<u32>,
  #[command(flatten)]
  page: PageArgs,
}

const EX_REVIEWS: &str = "\
Examples:
  atp steam reviews app/620 --sort recent -n 50
  atp steam reviews '#1' --type negative --language schinese
  atp steam reviews 1245620 --days 30 --purchase steam

More: atp guide steam reviews";

const EX_PRICES: &str = "\
Examples:
  atp steam prices 1245620 --cc us,cn,tr,ar
  atp steam prices app/620 1245620 '#3' --cc de

More: atp guide steam prices";

const EX_GAMES: &str = "\
Examples:
  atp steam games
  atp steam games gabelogannewell --recent
  atp steam games 76561197960287930 --sort name -n 100

More: atp guide steam games";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Reviews(args) => reviews(api, args).await?,
    Command::Prices { apps, cc } => Data::Posts(apps::prices(api, &apps, &cc).await?),
    Command::Games {
      user,
      recent,
      sort,
      page,
    } => {
      let user = user.map(|u| api.ctx.user_ref(&u)).transpose()?;
      let all = users::games(api, user.as_deref(), recent, &sort).await?;
      Data::Posts(
        page
          .collect(async |req| Ok(slice(all.clone(), &req)))
          .await?,
      )
    }
  })
}

async fn reviews(api: &Api, args: ReviewArgs) -> Result<Data> {
  let app = api.ctx.post_ref(&args.app)?;
  let filter = Filter {
    sort: args.sort,
    kind: args.kind,
    language: args.language,
    purchase: args.purchase,
    days: args.days,
  };
  let page = args
    .page
    .collect_dated(async |req| reviews::list(api, &app, &filter, &req).await)
    .await?;
  Ok(Data::Comments(page))
}
