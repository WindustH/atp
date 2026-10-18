//! 51job commands beyond the shared set: the job search with its filters
//! and the city codes.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::filters::{self, Code, CompanyType, Degree, Experience, JobType, Pay, Size};
use crate::jobs::{self, Filter};

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); every job without them
  pub words: Vec<String>,
  /// City: a name (北京, 杭州, 广东省 …) or its code from `cities`; all of China by default
  #[arg(short, long)]
  pub city: Option<String>,
  /// Monthly pay, in thousands of yuan
  #[arg(short, long, value_enum, value_name = "RANGE")]
  pub salary: Option<Pay>,
  /// Years of experience asked for
  #[arg(short, long, value_enum)]
  pub experience: Option<Experience>,
  /// Education asked for
  #[arg(short, long, value_enum)]
  pub degree: Option<Degree>,
  /// Who owns the company
  #[arg(long, value_enum, value_name = "TYPE")]
  pub company_type: Option<CompanyType>,
  /// How many people the company has
  #[arg(long, value_enum)]
  pub size: Option<Size>,
  /// Full-time, part-time or internship
  #[arg(short = 't', long = "type", value_enum, value_name = "TYPE")]
  pub job_type: Option<JobType>,
  /// Order: relevance (the site's), newest, salary, active (recruiters active of late)
  #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(filters::SORTS))]
  pub sort: Option<String>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the site's filters: city, pay, experience, degree, company, job type
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// City codes for `jobs --city`: the hot cities, or the places whose name contains NAME
  Cities {
    /// A place to look up (杭州, 广东, suzhou)
    name: Option<String>,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp 51job jobs rust --city 北京 -n 40
  atp 51job jobs 产品经理 --city 杭州 --salary 15-20k --experience 3-5
  atp 51job jobs --type intern --city 上海 --degree master 算法
  atp 51job jobs 会计 --city 广东省 --company-type state --sort newest

Cities are names or codes from `cities`.
More: atp guide 51job jobs";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Cities { name } => Data::Collections(Page::last(filters::list(name.as_deref()))),
  })
}

/// `jobs`: the filters as the site's codes.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = Filter {
    words: args.words.join(" "),
    salary: args.salary.map(Code::code),
    experience: args.experience.map(Code::code),
    degree: args.degree.map(Code::code),
    company_type: args.company_type.map(Code::code),
    size: args.size.map(Code::code),
    job_type: args.job_type.map(Code::code),
    sort: filters::sort(args.sort.as_deref()),
    ..Filter::default()
  };
  if let Some(city) = &args.city {
    filter.city = filters::city(city)?;
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
