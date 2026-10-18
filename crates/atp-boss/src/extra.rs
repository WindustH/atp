//! BOSS Zhipin commands beyond the shared set: the job search with its
//! filters, the codes of cities and industries, your applications,
//! interviews and chatted jobs, and applying to one job.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::filters::{self, Code, Degree, Experience, JobType, Pay, Size, Stage};
use crate::jobs::{self, Filter};
use crate::{apply, library};

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); the city's recommended jobs without them
  pub words: Vec<String>,
  /// City: a name (北京, 杭州 …), its pinyin or its code from `cities`; all of China by default
  #[arg(short, long)]
  pub city: Option<String>,
  /// Monthly pay, in thousands of yuan
  #[arg(short, long, value_enum, value_name = "RANGE")]
  pub salary: Option<Pay>,
  /// Experience asked for
  #[arg(short, long, value_enum)]
  pub experience: Option<Experience>,
  /// Education asked for
  #[arg(short, long, value_enum)]
  pub degree: Option<Degree>,
  /// Industry: a name (人工智能, 游戏 …) or its code from `industries`
  #[arg(short, long)]
  pub industry: Option<String>,
  /// How many people the company has
  #[arg(long, value_enum)]
  pub size: Option<Size>,
  /// The company's funding stage
  #[arg(long, value_enum)]
  pub stage: Option<Stage>,
  /// Full-time or part-time
  #[arg(short = 't', long = "type", value_enum, value_name = "TYPE")]
  pub job_type: Option<JobType>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the site's filters: city, pay, experience, degree, industry, company, job type
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// City codes for `jobs --city`: the hot cities, or the cities whose name or pinyin contains NAME
  Cities {
    /// A city to look up (杭州, suzhou)
    name: Option<String>,
  },
  /// Industries and their codes for `jobs --industry`
  Industries,
  /// Your applications (已投递; needs a login)
  Applied {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Interviews recruiters invited you to (面试; needs a login)
  Interviews {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Jobs whose recruiters you chatted with (沟通过; needs a login)
  Chatted {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Apply to one job: greet its recruiter with your default greeting (needs a login)
  #[command(after_long_help = EX_APPLY)]
  Apply {
    /// Job: a /job_detail/ link, the id in it, or #N from the last list
    job: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp boss jobs rust --city 北京
  atp boss jobs 产品经理 --city 杭州 --salary 20-50k --experience 3-5
  atp boss jobs 算法 --city shanghai --degree master --industry 人工智能
  atp boss jobs 运营 --city 深圳 --size 1000-9999 --stage listed

Cities and industries are names or codes from `cities` and `industries`.
Without a login the site shows the first 15 jobs of a search.
More: atp guide boss jobs";

const EX_APPLY: &str = "\
Examples:
  atp boss apply https://www.zhipin.com/job_detail/af620b46e682c71f0nZ409q8FVNV.html
  atp boss jobs rust --city 北京
  atp boss apply #3

Shows the job and asks before greeting; -y skips the question.
More: atp guide boss apply";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Cities { name } => Data::Collections(Page::last(filters::city_list(name.as_deref()))),
    Command::Industries => Data::Collections(Page::last(filters::industry_list())),
    Command::Applied { page } => Data::Posts(
      page
        .collect_dated(async |req| library::applied(api, &req).await)
        .await?,
    ),
    Command::Interviews { page } => Data::Posts(
      page
        .collect_dated(async |req| library::interviews(api, &req).await)
        .await?,
    ),
    Command::Chatted { page } => Data::Posts(
      page
        .collect_dated(async |req| library::chatted(api, &req).await)
        .await?,
    ),
    Command::Apply { job, yes } => {
      let job = api.ctx.post_ref(&job)?;
      Data::Action(apply::apply(api, &job, yes).await?)
    }
  })
}

/// `jobs`: the filters as the site's codes.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = Filter {
    words: args.words.join(" "),
    salary: args.salary.map(Code::code),
    experience: args.experience.map(Code::code),
    degree: args.degree.map(Code::code),
    size: args.size.map(Code::code),
    stage: args.stage.map(Code::code),
    job_type: args.job_type.map(Code::code),
    ..Filter::default()
  };
  if let Some(city) = &args.city {
    filter.city = filters::city(city)?;
  }
  if let Some(industry) = &args.industry {
    filter.industry = Some(filters::industry(industry)?);
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
