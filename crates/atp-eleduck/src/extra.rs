//! Eleduck commands beyond the shared set: the jobs channel with its
//! filters, categories and their tags, and your job applications.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::posts::JobFilter;
use crate::{categories, library, posts, refs};

/// The ways of hiring the jobs channel tells apart.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum JobKind {
  /// 社区帖子招聘: posts members write
  Community,
  /// 简历智能匹配: openings applied to with a résumé on the site
  Matched,
  /// 精选职位推荐: openings Eleduck's talent service recommends
  Featured,
}

impl JobKind {
  fn recruitment_type(self) -> &'static str {
    match self {
      Self::Community => "jd",
      Self::Matched => "jobs_channel",
      Self::Featured => "talent",
    }
  }
}

/// States of your applications, as the site's tabs group them.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ApplicationStatus {
  /// Waiting to be read or to be contacted
  Processing,
  /// Turned down
  Rejected,
  /// Taken back by you
  Withdrawn,
  /// Found a fit
  Qualified,
  /// Not read in time
  Expired,
}

impl ApplicationStatus {
  fn name(self) -> &'static str {
    match self {
      Self::Processing => "processing",
      Self::Rejected => "rejected",
      Self::Withdrawn => "withdrawn",
      Self::Qualified => "qualified",
      Self::Expired => "expired",
    }
  }
}

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Only one way of hiring
  #[arg(short, long, value_enum)]
  pub kind: Option<JobKind>,
  /// 工作方式: remote, onsite, part-time, offline-part-time, hybrid, or the tag's name or id
  #[arg(short = 't', long = "type", value_name = "MODE")]
  pub mode: Option<String>,
  /// 城市: a city tag (北京, 上海, 海外 …) or its id
  #[arg(long)]
  pub city: Option<String>,
  /// 职业: 开发, 产品, 设计, 运营, 写作, 运维, AI工程师, 其它, or the tag's id
  #[arg(long)]
  pub role: Option<String>,
  /// 招聘类型: 企业直招, 外包零活, 员工内推, 组队合伙, 猎头中介, or the tag's id
  #[arg(long)]
  pub hire: Option<String>,
  /// 领域: a field tag (AI/人工智能, 跨境/出海 …) or its id
  #[arg(long)]
  pub field: Option<String>,
  /// Newest first instead of the site's own order
  #[arg(long)]
  pub new: bool,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Job posts of the jobs channel (机会), narrowed by tags
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// The categories (版块) of the site
  Categories,
  /// The tags of a category, for the filters of `jobs`
  #[command(after_long_help = EX_TAGS)]
  Tags {
    /// Category: number, /categories/ link or #N after `categories` (the jobs channel's by default)
    category: Option<String>,
  },
  /// Your job applications and their state (needs a login)
  Applications {
    /// Only applications in this state
    #[arg(short, long, value_enum)]
    status: Option<ApplicationStatus>,
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp eleduck jobs -n 50
  atp eleduck jobs --type remote --role 开发 --new
  atp eleduck jobs --kind featured
  atp eleduck jobs --city 上海 --hire 企业直招

Tag names are as the site shows them; `atp eleduck tags` lists them.
More: atp guide eleduck jobs";

const EX_TAGS: &str = "\
Examples:
  atp eleduck tags
  atp eleduck categories
  atp eleduck tags 23

More: atp guide eleduck tags";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  let ctx = &api.ctx;
  Ok(match command {
    Command::Jobs(args) => Data::Posts(jobs(api, args).await?),
    Command::Categories => Data::Collections(Page::last(categories::list(api).await?)),
    Command::Tags { category } => {
      // A bare number is a category, not an item of the last list.
      let id = match category {
        Some(arg) if arg.trim().starts_with('#') => {
          Some(refs::category(&ctx.collection_ref(&arg)?)?)
        }
        Some(arg) => Some(refs::category(&arg)?),
        None => None,
      };
      Data::Collections(Page::last(categories::tags(api, id).await?))
    }
    Command::Applications { status, page } => {
      let status = status.map(ApplicationStatus::name);
      Data::Posts(
        page
          .collect(async |req| library::applications(api, status, &req).await)
          .await?,
      )
    }
  })
}

/// The jobs channel, its tags resolved to ids.
async fn jobs(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = JobFilter {
    kind: args.kind.map(JobKind::recruitment_type),
    sort: args.new.then_some("-published_at"),
    ..JobFilter::default()
  };
  let wanted = [
    ("job_type", args.mode),
    ("city", args.city),
    ("skill_type", args.role),
    ("hire_type", args.hire),
    ("field", args.field),
  ];
  for (group, value) in wanted {
    if let Some(value) = value {
      filter
        .tags
        .push(categories::job_tag(api, group, &value).await?);
    }
  }
  args
    .page
    .collect_dated(async |req| posts::jobs(api, &filter, &req).await)
    .await
}
