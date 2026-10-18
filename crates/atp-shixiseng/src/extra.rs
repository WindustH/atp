//! Shixiseng commands beyond the shared set: the job search with its
//! filters, applying to one job and your applications.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::apply::{self, Answers};
use crate::filters::{Degree, Kind, Months, Offer, Pay, Published, Stage, Start, Stay};
use crate::jobs::{self, Filter};
use crate::library;

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); every job without them
  pub words: Vec<String>,
  /// 实习 (intern, the default) or 校招 (campus)
  #[arg(short, long, value_enum)]
  pub kind: Option<Kind>,
  /// City as the site writes it (北京, 上海, 深圳 …); all of China by default
  #[arg(short, long)]
  pub city: Option<String>,
  /// Days a week at work, 1 to 6 (6 means six or more)
  #[arg(long, value_parser = clap::value_parser!(u8).range(1..=6))]
  pub days: Option<u8>,
  /// How many months the internship lasts
  #[arg(long, value_enum)]
  pub months: Option<Months>,
  /// Education asked for
  #[arg(short, long, value_enum)]
  pub degree: Option<Degree>,
  /// Daily pay in yuan
  #[arg(short, long, value_enum)]
  pub pay: Option<Pay>,
  /// Whether the internship may turn into a job (转正)
  #[arg(long, value_enum)]
  pub offer: Option<Offer>,
  /// Published within a day, a week or a month
  #[arg(long, value_enum)]
  pub published: Option<Published>,
  /// Only well-known companies (知名企业)
  #[arg(long)]
  pub top: bool,
  /// Newest first instead of the site's order
  #[arg(long)]
  pub new: bool,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search internships with the site's filters, or campus jobs: city, days a week, months, degree, pay
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// Apply to one job with your résumé on the site (needs a login)
  #[command(after_long_help = EX_APPLY)]
  Apply {
    /// Job: an /intern/ link, its id (inn_…) or #N from the last list
    job: String,
    /// Résumé, by the group id the confirmation lists, when the account has several
    #[arg(long, value_name = "ID")]
    resume: Option<String>,
    /// Days a week you can work, 1 to 7 (what the site fills in by default)
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=7))]
    days: Option<u8>,
    /// When you can start (what the site fills in by default)
    #[arg(long, value_enum)]
    start: Option<Start>,
    /// How long you can stay (what the site fills in by default)
    #[arg(long, value_enum)]
    stay: Option<Stay>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Your applications (投递记录) and their stage (needs a login)
  Applied {
    /// Only applications at this stage
    #[arg(short, long, value_enum)]
    stage: Option<Stage>,
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp shixiseng jobs 数据分析 --city 北京 -n 40
  atp shixiseng jobs java --city 上海 --days 3 --pay 200-300 --offer yes
  atp shixiseng jobs 产品 --published week --new
  atp shixiseng jobs --kind campus 算法 --city 杭州

Cities are names as the site writes them; the campus search takes words and a city only.
More: atp guide shixiseng jobs";

const EX_APPLY: &str = "\
Examples:
  atp shixiseng jobs 运营 --city 上海
  atp shixiseng apply '#3'
  atp shixiseng apply '#3' --days 4 --start 2w --stay 3-6m

Shows the job, the résumé and what goes with it, and asks before sending; -y skips the question.
More: atp guide shixiseng apply";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Apply {
      job,
      resume,
      days,
      start,
      stay,
      yes,
    } => {
      let job = api.ctx.post_ref(&job)?;
      let answers = Answers {
        resume,
        days,
        start,
        stay,
      };
      Data::Action(apply::apply(api, &job, &answers, yes).await?)
    }
    Command::Applied { stage, page } => {
      let stage = stage.map_or("delivered", Stage::code);
      Data::Posts(
        page
          .collect(async |req| library::applied(api, stage, &req).await)
          .await?,
      )
    }
  })
}

/// `jobs`: the filters as the search takes them.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = Filter {
    words: args.words.join(" "),
    kind: args.kind.unwrap_or_default(),
    days: args.days,
    months: args.months,
    degree: args.degree,
    pay: args.pay,
    offer: args.offer,
    published: args.published,
    top: args.top,
    newest: args.new,
    ..Filter::default()
  };
  if let Some(city) = args.city.filter(|c| !c.trim().is_empty()) {
    filter.city = city.trim().to_owned();
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
