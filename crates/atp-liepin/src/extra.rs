//! Liepin commands beyond the shared set: the job search with every
//! filter, the codes of cities and industries, your applications and
//! invitations, and applying to one job.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::filters::{self, Degree, Experience, JobKind, Salary, Size, Stage};
use crate::jobs::{self, Filter};
use crate::{apply, library};

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); all jobs without them
  pub words: Vec<String>,
  /// City: a name (北京, 杭州, 厦门 …) or its code from `cities`; all of China by default
  #[arg(short, long)]
  pub city: Option<String>,
  /// Monthly pay
  #[arg(short, long, value_enum)]
  pub salary: Option<Salary>,
  /// Years of experience asked for
  #[arg(short, long, value_enum)]
  pub experience: Option<Experience>,
  /// Education asked for
  #[arg(short, long, value_enum)]
  pub degree: Option<Degree>,
  /// Industry: a name (人工智能, 互联网 …), a group's name or a code from `industries`
  #[arg(short, long)]
  pub industry: Option<String>,
  /// Company size (people)
  #[arg(long, value_enum)]
  pub size: Option<Size>,
  /// Company funding stage
  #[arg(long, value_enum)]
  pub stage: Option<Stage>,
  /// Only jobs posted by headhunters or by companies themselves
  #[arg(short, long, value_enum)]
  pub kind: Option<JobKind>,
  #[command(flatten)]
  pub page: PageArgs,
}

/// States of your applications, as the site's tabs group them.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ApplicationStatus {
  /// Sent, not yet read (已投递)
  Applied,
  /// Read by the recruiter (已查看)
  Viewed,
  /// Invited to an interview (面试邀约)
  Interview,
  /// Turned down (不合适)
  Rejected,
}

impl ApplicationStatus {
  fn name(self) -> &'static str {
    match self {
      Self::Applied => "applied",
      Self::Viewed => "viewed",
      Self::Interview => "interview",
      Self::Rejected => "rejected",
    }
  }
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the site's filters: city, pay, experience, degree, industry, company
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// City codes for `jobs --city`: the hot cities, or the places matching a name
  #[command(after_long_help = EX_CITIES)]
  Cities {
    /// A city, province or district to look up
    name: Option<String>,
  },
  /// Industries and their codes for `jobs --industry`
  Industries,
  /// Your applications (我的投递) and their state (needs a login)
  Applied {
    /// Only applications in this state
    #[arg(short, long, value_enum)]
    status: Option<ApplicationStatus>,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Invitations to apply that recruiters sent you (邀请应聘; needs a login)
  Invitations {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Apply to one job with your default online résumé (needs a login)
  #[command(after_long_help = EX_APPLY)]
  Apply {
    /// Job: a /job/ or /a/ link, the number in it, or #N from the last list
    job: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp liepin jobs rust --city 上海 -n 80
  atp liepin jobs 产品经理 --city 杭州 --salary 20-40k --experience 3-5
  atp liepin jobs --city 北京 --industry 人工智能 --degree master --kind company
  atp liepin jobs 数据分析 --size 10000+ --stage listed

Names of cities and industries are as the site shows them; `cities` and
`industries` list the codes.
More: atp guide liepin jobs";

const EX_CITIES: &str = "\
Examples:
  atp liepin cities
  atp liepin cities 厦门

More: atp guide liepin cities";

const EX_APPLY: &str = "\
Examples:
  atp liepin apply https://www.liepin.com/job/1985552821.shtml
  atp liepin jobs rust --city 上海
  atp liepin apply #3

Shows the job and asks before sending; -y skips the question.
More: atp guide liepin apply";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Cities { name } => {
      let cities = match name {
        Some(name) => filters::find_cities(api, &name).await?,
        None => filters::hot_cities(api).await?,
      };
      Data::Collections(Page::last(cities))
    }
    Command::Industries => Data::Collections(Page::last(filters::industries(api).await?)),
    Command::Applied { status, page } => {
      let status = status.map(ApplicationStatus::name);
      Data::Posts(
        page
          .collect(async |req| library::applied(api, status, &req).await)
          .await?,
      )
    }
    Command::Invitations { page } => Data::Posts(
      page
        .collect(async |req| library::invitations(api, &req).await)
        .await?,
    ),
    Command::Apply { job, yes } => {
      let job = api.ctx.post_ref(&job)?;
      Data::Action(apply::apply(api, &job, yes).await?)
    }
  })
}

/// `jobs`: the filters resolved to the site's codes.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = Filter {
    key: args.words.join(" "),
    salary: args.salary.map(Salary::code).unwrap_or_default().into(),
    experience: args
      .experience
      .map(Experience::code)
      .unwrap_or_default()
      .into(),
    degree: args.degree.map(Degree::code).unwrap_or_default().into(),
    size: args.size.map(Size::code).unwrap_or_default().into(),
    stage: args.stage.map(Stage::code).unwrap_or_default().into(),
    kind: args.kind.map(JobKind::code).unwrap_or_default().into(),
    ..Filter::default()
  };
  if let Some(city) = &args.city {
    filter.city = filters::city(api, city).await?;
  }
  if let Some(industry) = &args.industry {
    filter.industry = filters::industry(api, industry).await?;
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
