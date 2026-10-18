//! LinkedIn commands beyond the shared set: the job search with what
//! LinkedIn's search narrows by for a visitor.

use atp_core::cli::PageArgs;
use atp_core::{Data, Error, Page, Post, Result};

use crate::api::Api;
use crate::filters::{Filter, Posted};
use crate::{jobs, users};

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); every job without them
  pub words: Vec<String>,
  /// Where: a place in words, as LinkedIn's search box takes it (Berlin, "San Francisco Bay Area", Japan …)
  #[arg(short, long, conflicts_with = "geo")]
  pub location: Option<String>,
  /// Where, by LinkedIn's number for a place (`geoId` in its links; 92000000 is the whole world)
  #[arg(long, value_name = "ID")]
  pub geo: Option<String>,
  /// Posted in the past day, week or month
  #[arg(short, long, value_enum)]
  pub posted: Option<Posted>,
  /// Company: its link, the name in it or its number; several separated by commas
  #[arg(short, long, value_delimiter = ',')]
  pub company: Vec<String>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs by words, place, date posted and company
  #[command(after_long_help = EX_JOBS)]
  Jobs(Box<JobArgs>),
}

const EX_JOBS: &str = "\
Examples:
  atp linkedin jobs rust --location Berlin --posted week
  atp linkedin jobs 'data engineer' -l 'United States' -n 50
  atp linkedin jobs --company https://www.linkedin.com/company/nvidia --posted month
  atp linkedin jobs designer --geo 92000000 --posted day
  atp linkedin read '#2'

Places are words, as in LinkedIn's own search box; --geo takes the geoId of
its links. LinkedIn shows the first 1000 jobs of a search at most, and does
not narrow a visitor's search by workplace, experience or job type.
More: atp guide linkedin jobs";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, *args).await?),
  })
}

/// `jobs`: the filters as LinkedIn's search takes them, companies by
/// their numbers.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  if let Some(geo) = &args.geo
    && !geo.trim().bytes().all(|b| b.is_ascii_digit())
  {
    return Err(Error::input(format!(
      "--geo takes LinkedIn's number for a place (geoId), not {geo}; give words with --location"
    )));
  }
  let mut companies = Vec::new();
  for company in args.company.iter().filter(|c| !c.trim().is_empty()) {
    companies.push(users::job_ids(api, company).await?);
  }
  let filter = Filter {
    words: args.words.join(" ").trim().to_owned(),
    location: args.location,
    geo: args.geo.map(|g| g.trim().to_owned()),
    posted: args.posted,
    companies,
  };
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
