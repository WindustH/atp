//! Nowcoder commands beyond the shared set: the job search with its
//! filters and the career categories it takes, the 校招 schedule and
//! subscribing to its entries, 面经, 内推 and your applications.

use atp_core::cli::PageArgs;
use atp_core::{Data, Error, Page, Post, Result};

use crate::api::Api;
use crate::filters::{self, JobKind, Nature, Salary, Size, Tab};
use crate::jobs::{self, Filter};
use crate::posts::{self, Experience};
use crate::schedule;

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill, a company); every job without them
  pub words: Vec<String>,
  /// 校招 (campus, the default), 实习 (intern) or 社招 (social)
  #[arg(short, long, value_enum)]
  pub kind: Option<JobKind>,
  /// City (北京, 上海 …); several give jobs in any of them
  #[arg(short, long)]
  pub city: Vec<String>,
  /// Career category of the narrowest level (Java, golang, 前端 …) or its id from `careers`
  #[arg(long)]
  pub career: Option<String>,
  /// Graduation year: 2027 or 2027届; several give any of them
  #[arg(long)]
  pub year: Vec<String>,
  /// Monthly pay (campus and experienced jobs)
  #[arg(short, long, value_enum)]
  pub salary: Option<Salary>,
  /// Who owns the company (国企, 央企, 银行, 研究所, 外企, 民企, 事业单位 too)
  #[arg(long, value_enum)]
  pub nature: Option<Nature>,
  /// Company size (people); several give any of them
  #[arg(long, value_enum)]
  pub size: Vec<Size>,
  /// Newest first instead of the site's recommendation
  #[arg(long)]
  pub new: bool,
  #[command(flatten)]
  pub page: PageArgs,
}

/// What `schedule` lists.
#[derive(Debug, clap::Args)]
pub struct ScheduleArgs {
  /// A company to look for; every company without it
  pub words: Vec<String>,
  /// The page's tab
  #[arg(short, long, value_enum, default_value = "recommended")]
  pub tab: Tab,
  /// City (北京, 上海 …); several give any of them
  #[arg(short, long)]
  pub city: Vec<String>,
  /// Career category of the middle level (后端开发, 产品经理 …; Java stands for 后端开发) or its id
  #[arg(long)]
  pub career: Option<String>,
  /// Industry (人工智能, 银行, 游戏 …) or its id; several give any of them
  #[arg(short, long)]
  pub industry: Vec<String>,
  /// Company type: 国企, 外企, 民企, 事业单位, or its id
  #[arg(long = "type", value_name = "TYPE")]
  pub company_type: Option<String>,
  /// Hiring batch: 27届秋招, 27届实习, 日常实习 …, or its id
  #[arg(short, long)]
  pub batch: Option<String>,
  #[command(flatten)]
  pub page: PageArgs,
}

/// What `experience` lists.
#[derive(Debug, clap::Args)]
pub struct ExperienceArgs {
  /// Company, by its name as the site writes it or its id; several give any of them
  #[arg(short, long)]
  pub company: Vec<String>,
  /// Career category: a name or an id from `careers` (the site's first, 软件开发, by default)
  #[arg(long)]
  pub career: Option<String>,
  /// Newest first instead of the site's order (综合)
  #[arg(long)]
  pub new: bool,
  #[command(flatten)]
  pub page: PageArgs,
}

/// Stages of your applications, as the site's tabs group them.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Stage {
  /// Read by the company (简历被查看)
  Viewed,
  /// Past the first screening (通过初筛)
  Passed,
  /// Turned down (不合适)
  Rejected,
}

impl Stage {
  fn code(self) -> i64 {
    match self {
      Self::Viewed => 7,
      Self::Passed => 1,
      Self::Rejected => 2,
    }
  }
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the site's filters: kind, city, career, graduation year, pay, company
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// Career categories and their ids, for --career
  Careers {
    /// Only the categories whose name or group contains this
    name: Option<String>,
  },
  /// The 校招 schedule (校招日程): when companies take applications, referral codes, application pages
  #[command(after_long_help = EX_SCHEDULE)]
  Schedule(ScheduleArgs),
  /// Subscribe to a company's schedule entry (收藏; needs a login)
  ScheduleSubscribe {
    /// Company: its number, a /jobs/school/schedule/ link or #N after `schedule`
    company: String,
  },
  /// Stop a subscription to a company's schedule entry (needs a login)
  ScheduleUnsubscribe {
    /// Company: its number, a /jobs/school/schedule/ link or #N after `schedule`
    company: String,
  },
  /// Interview write-ups (面经) by career and company
  #[command(after_long_help = EX_EXPERIENCE)]
  Experience(ExperienceArgs),
  /// Referral posts (内推) of the home page (needs a login)
  Referral {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Your job applications (投递记录) and their stage (needs a login)
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
  atp nowcoder jobs 后端 --city 上海 --year 2027 -n 60
  atp nowcoder jobs --kind intern --career Java --city 北京 --city 杭州
  atp nowcoder jobs 算法 --salary 30-50k --nature state --new
  atp nowcoder jobs --kind social rust

Cities are names as the site writes them; `careers` lists the categories.
More: atp guide nowcoder jobs";

const EX_SCHEDULE: &str = "\
Examples:
  atp nowcoder schedule -n 40
  atp nowcoder schedule --tab closing --batch 27届秋招
  atp nowcoder schedule 银行 --city 上海 --type 国企
  atp nowcoder schedule-subscribe '#3'

More: atp guide nowcoder schedule";

const EX_EXPERIENCE: &str = "\
Examples:
  atp nowcoder experience --career 后端开发 --new
  atp nowcoder experience --company 字节跳动 --career Java
  atp nowcoder read '#1'

More: atp guide nowcoder experience";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  let ctx = &api.ctx;
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Careers { name } => {
      Data::Collections(Page::last(filters::careers(api, name.as_deref()).await?))
    }
    Command::Schedule(args) => Data::Posts(schedule_list(api, args).await?),
    Command::ScheduleSubscribe { company } => {
      Data::Action(schedule::subscribe(api, &ctx.post_ref(&company)?, false).await?)
    }
    Command::ScheduleUnsubscribe { company } => {
      Data::Action(schedule::subscribe(api, &ctx.post_ref(&company)?, true).await?)
    }
    Command::Experience(args) => Data::Posts(experience(api, args).await?),
    Command::Referral { page } => Data::Posts(
      page
        .collect_dated(async |req| posts::referral(api, &req).await)
        .await?,
    ),
    Command::Applied { stage, page } => {
      let stage = stage.map_or(0, Stage::code);
      Data::Posts(
        page
          .collect(async |req| jobs::applied(api, stage, &req).await)
          .await?,
      )
    }
  })
}

/// `2027` → `2027届`, as the site names graduation years.
fn year(value: &str) -> String {
  let value = value.trim();
  if value.bytes().all(|b| b.is_ascii_digit()) {
    format!("{value}届")
  } else {
    value.to_owned()
  }
}

/// `jobs`: the filters resolved to the site's codes.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let kind = args.kind.unwrap_or(JobKind::Campus);
  if args.salary.is_some() && matches!(kind, JobKind::Intern) {
    return Err(Error::input(
      "--salary filters campus and experienced jobs by monthly pay; internships are paid by the day",
    ));
  }
  let mut filter = Filter {
    query: args.words.join(" "),
    kind: kind.code(),
    cities: args.city,
    years: args.year.iter().map(|y| year(y)).collect(),
    salary: args.salary.map(Salary::range),
    nature: args.nature.map(Nature::code),
    sizes: args.size.into_iter().map(Size::code).collect(),
    newest: args.new,
    ..Filter::default()
  };
  if let Some(career) = &args.career {
    filter.career = Some(filters::career(api, career, Some(3)).await?.id);
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}

/// `schedule`: the filters resolved to the site's codes.
async fn schedule_list(api: &Api, args: ScheduleArgs) -> Result<Page<Post>> {
  let mut filter = schedule::Filter {
    query: args.words.join(" "),
    tab: args.tab.code(),
    cities: args.city,
    ..schedule::Filter::default()
  };
  if let Some(career) = &args.career {
    filter.career = Some(filters::career(api, career, Some(2)).await?.id);
  }
  for industry in &args.industry {
    filter
      .industries
      .push(filters::industry(api, industry).await?);
  }
  if let Some(kind) = &args.company_type {
    filter.company_type = Some(filters::company_type(api, kind).await?);
  }
  if let Some(batch) = &args.batch {
    filter.batch = Some(filters::batch(api, batch).await?);
  }
  args
    .page
    .collect(async |req| schedule::list(api, &filter, &req).await)
    .await
}

/// `experience`: companies and the career resolved to ids.
async fn experience(api: &Api, args: ExperienceArgs) -> Result<Page<Post>> {
  let mut filter = Experience {
    companies: Vec::new(),
    career: None,
    newest: args.new,
  };
  for company in &args.company {
    filter
      .companies
      .push(posts::experience_company(api, company).await?);
  }
  if let Some(career) = &args.career {
    filter.career = Some(filters::career(api, career, None).await?);
  }
  args
    .page
    .collect_dated(async |req| posts::experience(api, &filter, &req).await)
    .await
}
