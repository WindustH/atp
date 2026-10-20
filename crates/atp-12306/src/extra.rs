//! 12306's commands: stations, the trains between two stations, a
//! train's stops and its fares. All read-only.

use atp_core::cli::PageArgs;
use atp_core::paging::slice;
use atp_core::{Data, Error, Page, Result};
use jiff::civil::Date;

use crate::api::Api;
use crate::parse;
use crate::refs;
use crate::stations::Stations;
use crate::trains::{self, Filter};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Stations matching a name, telecode, pinyin or city
  #[command(after_long_help = EX_STATIONS)]
  Stations {
    /// Words to look up: 上海, 虹桥, AOH, shanghai, shhq
    keyword: String,
    #[command(flatten)]
    page: PageArgs,
  },
  /// The day's trains between two stations, with the seats left in each class
  #[command(visible_alias = "tickets", after_long_help = EX_TRAINS)]
  Trains(TrainsArgs),
  /// A train's stops: arrival, departure and stop at each station
  #[command(visible_alias = "stops", after_long_help = EX_TRAIN)]
  Train {
    /// Train: a code (G102), or #N / an id of a `trains` list
    train: String,
    /// YYYY-MM-DD (the day of the list's train, else today in Beijing)
    #[arg(long)]
    date: Option<String>,
  },
  /// A train's fares between two of its stations, per seat class
  #[command(after_long_help = EX_PRICE)]
  Price {
    /// Train: a code (G102), or #N / an id of a `trains` list
    train: String,
    /// From this station (the list's one for a train of a `trains` list)
    from: Option<String>,
    /// To this station
    to: Option<String>,
    /// YYYY-MM-DD (the day of the list's train, else today in Beijing)
    #[arg(long)]
    date: Option<String>,
  },
}

#[derive(Debug, clap::Args)]
pub struct TrainsArgs {
  /// From: a station (上海虹桥), a city's main station (上海), a telecode (AOH) or pinyin
  from: String,
  /// To: as FROM
  to: String,
  /// YYYY-MM-DD (today in Beijing by default; 12306 sells about two weeks ahead)
  #[arg(long)]
  date: Option<String>,
  /// Only these kinds of train, by the code's letter (G, D, C, Z, T, K …), comma-separated
  #[arg(long = "type", value_name = "LETTERS", value_delimiter = ',')]
  kinds: Vec<String>,
  /// Only trains with a seat left
  #[arg(long)]
  available: bool,
}

const EX_STATIONS: &str = "\
Examples:
  atp 12306 stations 上海
  atp 12306 stations hongqiao

More: atp guide 12306 stations";

const EX_TRAINS: &str = "\
Examples:
  atp 12306 trains 上海 北京 --date 2026-10-25
  atp 12306 trains 上海虹桥 杭州东 --type G,D --available
  atp 12306 tickets AOH HGH

More: atp guide 12306 trains";

const EX_TRAIN: &str = "\
Examples:
  atp 12306 train G102 --date 2026-10-25
  atp 12306 train '#3'

More: atp guide 12306 train";

const EX_PRICE: &str = "\
Examples:
  atp 12306 price '#3'
  atp 12306 price G102 上海虹桥 北京南 --date 2026-10-25

More: atp guide 12306 price";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Stations { keyword, page } => {
      let stations = Stations::load(api).await?;
      let found: Vec<_> = stations
        .find(&keyword)
        .into_iter()
        .map(parse::station)
        .collect();
      Data::Posts(
        page
          .collect(async |req| Ok(slice(found.clone(), &req)))
          .await?,
      )
    }
    Command::Trains(args) => trains(api, args).await?,
    Command::Train { train, date } => {
      let train = refs::train(&api.ctx.post_ref(&train)?)?;
      let date = day(date.as_deref(), train.date)?;
      Data::Post(Box::new(trains::stops(api, &train, date).await?))
    }
    Command::Price {
      train,
      from,
      to,
      date,
    } => price(api, &train, from, to, date.as_deref()).await?,
  })
}

async fn trains(api: &Api, args: TrainsArgs) -> Result<Data> {
  let date = day(args.date.as_deref(), None)?;
  let filter = Filter {
    kinds: args
      .kinds
      .iter()
      .flat_map(|k| k.trim().chars())
      .map(|c| c.to_ascii_uppercase())
      .collect(),
    available: args.available,
  };
  let list = trains::list(api, &args.from, &args.to, date, &filter).await?;
  Ok(Data::Posts(Page::last(list)))
}

async fn price(
  api: &Api,
  train: &str,
  from: Option<String>,
  to: Option<String>,
  date: Option<&str>,
) -> Result<Data> {
  let train = refs::train(&api.ctx.post_ref(train)?)?;
  let date = day(date, train.date)?;
  let (from, to) = match (from, to, &train.from, &train.to) {
    (Some(from), Some(to), _, _) => (from, to),
    (None, None, Some(from), Some(to)) => (from.clone(), to.clone()),
    _ => {
      return Err(Error::input(
        "price needs FROM and TO, unless the train comes from a `trains` list",
      ));
    }
  };
  Ok(Data::Post(Box::new(
    trains::fares(api, &train, &from, &to, date).await?,
  )))
}

/// `--date`, else the day a list's train carries, else today.
fn day(given: Option<&str>, carried: Option<Date>) -> Result<Date> {
  match given {
    Some(text) => refs::day(text),
    None => Ok(carried.unwrap_or_else(refs::today)),
  }
}
