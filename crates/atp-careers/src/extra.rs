//! Careers commands beyond the shared set: the sites ATP reads, the job
//! search with each site's filters, and the codes those filters take.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::jobs::{self, Codes, Filter, Kind};
use crate::sites;

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill); every job without them
  pub words: Vec<String>,
  /// 校招 (campus), 实习 (intern) or 社招 (social); without it, what the site lists by default
  #[arg(short, long, value_enum)]
  pub kind: Option<Kind>,
  /// City, by name (北京, 深圳 …) or by the site's code from `cities`
  #[arg(short, long)]
  pub city: Option<String>,
  /// Batch or project, by its id from `batches`
  #[arg(short, long, value_name = "ID")]
  pub batch: Option<String>,
  /// Category, by name (后端, 技术类 …) or by its id from `categories`
  #[arg(long)]
  pub category: Option<String>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// The companies ATP knows by a short name, and the vendors whose sites it reads by their link
  Sites,
  /// Search the jobs of the --site: words, kind of hiring, city, batch, category
  #[command(after_long_help = EX_JOBS)]
  Jobs(Box<JobArgs>),
  /// The batches or projects of the --site (2027届校招, 日常实习 …), for jobs --batch
  #[command(after_long_help = EX_CODES)]
  Batches {
    /// The kind of hiring whose batches to list, where the site keeps them apart
    #[arg(short, long, value_enum)]
    kind: Option<Kind>,
  },
  /// The cities of the --site's filter, for jobs --city
  Cities {
    /// The kind of hiring whose cities to list, where the site keeps them apart
    #[arg(short, long, value_enum)]
    kind: Option<Kind>,
  },
  /// The categories of the --site's filter, for jobs --category
  Categories {
    /// The kind of hiring whose categories to list, where the site keeps them apart
    #[arg(short, long, value_enum)]
    kind: Option<Kind>,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp careers jobs --site bytedance 后端 --city 深圳 --kind intern
  atp careers jobs --site tencent --kind social 推荐算法
  atp careers jobs --site alibaba --batch 100000560002
  atp careers jobs --site 'https://app.mokahr.com/campus-recruitment/zhongan/166735' -n 50
  atp careers read '#3'

--site takes a short name (atp careers sites) or a career site's link, such as the
application links of Nowcoder's 校招 schedule.
More: atp guide careers jobs";

const EX_CODES: &str = "\
Examples:
  atp careers batches --site alibaba
  atp careers cities --site tencent
  atp careers categories --site kuaishou
  atp careers jobs --site alibaba --batch 100000560002 --category 技术类

More: atp guide careers batches";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Sites => Data::Collections(Page::last(sites::listing())),
    Command::Jobs(args) => Data::Posts(search(api, *args).await?),
    Command::Batches { kind } => codes(api, Codes::Batches, kind).await?,
    Command::Cities { kind } => codes(api, Codes::Cities, kind).await?,
    Command::Categories { kind } => codes(api, Codes::Categories, kind).await?,
  })
}

async fn codes(api: &Api, codes: Codes, kind: Option<Kind>) -> Result<Data> {
  Ok(Data::Collections(Page::last(
    jobs::codes(api, codes, kind).await?,
  )))
}

/// `jobs`: the filters as the site's search takes them.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let site = sites::current(api).await?;
  let some = |s: Option<String>| s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
  let filter = Filter {
    words: args.words.join(" ").trim().to_owned(),
    kind: args.kind,
    city: some(args.city),
    batch: some(args.batch),
    category: some(args.category),
  };
  args
    .page
    .collect_dated(async |req| jobs::search(api, &site, &filter, &req).await)
    .await
}
