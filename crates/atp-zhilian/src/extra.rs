//! Zhilian commands beyond the shared set: the job search with its
//! filters, the city codes and applying to one job.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::apply;
use crate::filters::{self, Degree, Experience, JobType};
use crate::jobs::{self, Filter};

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); every job without them
  pub words: Vec<String>,
  /// City: a name (北京, 杭州 …) or its code from `cities`; all of China by default
  #[arg(short, long)]
  pub city: Option<String>,
  /// Monthly pay in thousands of yuan, as a range: 15-25 (or 15k-25k)
  #[arg(short, long, value_name = "MIN-MAX")]
  pub salary: Option<String>,
  /// Years of experience asked for
  #[arg(short, long, value_enum)]
  pub experience: Option<Experience>,
  /// Education asked for
  #[arg(short, long, value_enum)]
  pub degree: Option<Degree>,
  /// Full-time, part-time, internship or campus hiring
  #[arg(short = 't', long = "type", value_enum, value_name = "TYPE")]
  pub job_type: Option<JobType>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the site's filters: city, pay, experience, degree, job type
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// City codes for `jobs --city`: the hot cities, or the cities whose name contains NAME
  Cities {
    /// A city to look up (杭州, 苏州)
    name: Option<String>,
  },
  /// Apply to one job with your résumé on the site (needs a login)
  #[command(after_long_help = EX_APPLY)]
  Apply {
    /// Job: a /jobs/ link, its number (CC…J…) or #N from the last list
    job: String,
    /// Résumé number, when the account has several (`whoami --raw` lists them)
    #[arg(long, value_name = "NUMBER")]
    resume: Option<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp zhilian jobs rust --city 北京 -n 40
  atp zhilian jobs 产品经理 --city 杭州 --salary 15-25 --experience 3-5
  atp zhilian jobs --type intern --city 上海 --degree master 算法
  atp zhilian jobs --type campus --city 成都

Cities are names or codes from `cities`.
More: atp guide zhilian jobs";

const EX_APPLY: &str = "\
Examples:
  atp zhilian jobs rust --city 北京
  atp zhilian apply '#3'

Shows the job and asks before sending; -y skips the question.
More: atp guide zhilian apply";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Cities { name } => Data::Collections(Page::last(filters::list(name.as_deref()))),
    Command::Apply { job, resume, yes } => {
      let job = api.ctx.post_ref(&job)?;
      Data::Action(apply::apply(api, &job, resume.as_deref(), yes).await?)
    }
  })
}

/// `jobs`: the filters resolved to the site's codes.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = Filter {
    words: args.words.join(" "),
    experience: args.experience.map(|e| e.code().to_owned()),
    degree: args.degree.map(|d| d.code().to_owned()),
    job_type: args.job_type,
    ..Filter::default()
  };
  if let Some(city) = &args.city {
    filter.city = filters::city(city)?;
  }
  if let Some(salary) = &args.salary {
    filter.salary = Some(filters::salary(salary)?);
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
