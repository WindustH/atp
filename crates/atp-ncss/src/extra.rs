//! NCSS commands beyond the shared set: the job search with its filters
//! and the codes they take, job fairs with their jobs and companies, and
//! applying to one job.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::fairs::{self, FairList};
use crate::filters::{self, Degree, JobType, Kind, Property, Salary, Source};
use crate::jobs::{self, Filter};
use crate::{apply, refs};

/// The filters the job search and a fair's jobs share.
#[derive(Debug, clap::Args)]
pub struct Filters {
  /// Area: a province, city or district (北京, 杭州, 海淀区) or its code from `areas`; all of China by default
  #[arg(short, long)]
  pub city: Option<String>,
  /// Full-time, part-time or internship
  #[arg(short = 't', long = "type", value_enum, value_name = "TYPE")]
  pub job_type: Option<JobType>,
  /// Monthly pay
  #[arg(short, long, value_enum)]
  pub salary: Option<Salary>,
  /// Job category (计算机, 销售类 …) or its code from `categories`
  #[arg(long)]
  pub category: Option<String>,
  /// Industry (计算机软件, 银行 …) or its code from `industries`; several (up to ten) give any of them
  #[arg(short, long)]
  pub industry: Vec<String>,
  /// Who owns the company (公司性质)
  #[arg(long, value_enum)]
  pub property: Option<Property>,
}

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a job title or a company); every job without them
  pub words: Vec<String>,
  #[command(flatten)]
  pub filters: Filters,
  /// Single jobs, or employers' announcements of several (公告)
  #[arg(short, long, value_enum)]
  pub kind: Option<Kind>,
  /// The education asked for, at least
  #[arg(short, long, value_enum)]
  pub degree: Option<Degree>,
  /// Where the job was posted (职位来源)
  #[arg(long, value_enum)]
  pub source: Option<Source>,
  /// Only companies of key fields (重点领域)
  #[arg(long)]
  pub key_field: bool,
  /// Only the platform's featured companies (精选企业)
  #[arg(long)]
  pub featured: bool,
  #[command(flatten)]
  pub page: PageArgs,
}

/// What `fair` lists.
#[derive(Debug, clap::Args)]
pub struct FairArgs {
  /// Job fair: a fairdetails.html?fairId= link, its id or #N after `fairs`
  pub fair: String,
  /// Search words (a job title or a company); every job of the fair without them
  pub words: Vec<String>,
  #[command(flatten)]
  pub filters: Filters,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the site's filters: area, type, pay, degree, category, industry, company, source
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// Area codes for --city: the provinces, or the places whose name contains NAME
  Areas {
    /// A city or district to look up (杭州, 海淀)
    name: Option<String>,
  },
  /// Job categories and their codes, for --category
  Categories,
  /// Industries and their codes, for --industry
  Industries,
  /// Job fairs (专场招聘): the site's online fairs, fairs around the country, the 24365 campaign
  #[command(after_long_help = EX_FAIRS)]
  Fairs {
    /// Which fairs
    #[arg(short, long, value_enum, default_value = "online")]
    list: FairList,
    #[command(flatten)]
    page: PageArgs,
  },
  /// The jobs of one of the site's job fairs, with the job search's filters
  Fair(FairArgs),
  /// The companies of one of the site's job fairs
  FairCompanies {
    /// Job fair: a fairdetails.html?fairId= link, its id or #N after `fairs`
    fair: String,
    /// Only companies whose name contains this
    name: Option<String>,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Apply to one job with the résumé you keep on the site (needs a login)
  #[command(after_long_help = EX_APPLY)]
  Apply {
    /// Job: a /student/jobs/<id>/detail.html link, its id or #N from the last list
    job: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp ncss jobs java --city 杭州 -n 40
  atp ncss jobs --type intern --city 北京 --category 计算机
  atp ncss jobs 管培生 --degree bachelor --salary 10-15k --property state
  atp ncss jobs --kind announcement --source platform

Areas are names or codes from `areas`; without a login the site shows ten pages.
More: atp guide ncss jobs";

const EX_FAIRS: &str = "\
Examples:
  atp ncss fairs
  atp ncss fairs --list 24365
  atp ncss fair '#1' 软件 --city 重庆
  atp ncss fair-companies '#1' 银行

More: atp guide ncss fairs";

const EX_APPLY: &str = "\
Examples:
  atp ncss jobs rust --city 上海
  atp ncss apply '#3'

Shows the job and asks before sending; -y skips the question.
More: atp guide ncss apply";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  let ctx = &api.ctx;
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Areas { name } => {
      Data::Collections(Page::last(filters::areas(api, name.as_deref()).await?))
    }
    Command::Categories => Data::Collections(Page::last(filters::categories())),
    Command::Industries => Data::Collections(Page::last(filters::industries())),
    Command::Fairs { list, page } => Data::Posts(
      page
        .collect_dated(async |req| fairs::list(api, list, &req).await)
        .await?,
    ),
    Command::Fair(args) => Data::Posts(fair_jobs(api, args).await?),
    Command::FairCompanies { fair, name, page } => {
      let fair = ctx.post_ref(&fair)?;
      Data::Users(
        page
          .collect(async |req| fairs::companies(api, &fair, name.as_deref(), &req).await)
          .await?,
      )
    }
    Command::Apply { job, yes } => {
      Data::Action(apply::apply(api, &ctx.post_ref(&job)?, yes).await?)
    }
  })
}

/// The shared filters resolved to the site's codes.
async fn resolve(api: &Api, words: Vec<String>, filters: Filters) -> Result<Filter> {
  let mut filter = Filter {
    words: words.join(" "),
    job_type: filters.job_type,
    salary: filters.salary.map_or("", Salary::code).to_owned(),
    property: filters.property.map_or("", Property::code).to_owned(),
    ..Filter::default()
  };
  if let Some(city) = &filters.city {
    filter.area = filters::area_code(api, city).await?;
  }
  if let Some(category) = &filters.category {
    filter.category = filters::category(category)?;
  }
  for industry in &filters.industry {
    filter.industries.push(filters::industry(industry)?);
  }
  Ok(filter)
}

/// `jobs`: the filters resolved to the site's codes.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = resolve(api, args.words, args.filters).await?;
  filter.kind = args.kind.map_or("", Kind::code).to_owned();
  filter.degree = args.degree.map_or("", Degree::code).to_owned();
  filter.key_field = args.key_field;
  filter.featured = args.featured;
  if let Some(source) = args.source {
    let (kind, name) = source.codes();
    filter.source = (kind.to_owned(), name.to_owned());
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}

/// `fair`: a fair's jobs with the filters resolved.
async fn fair_jobs(api: &Api, args: FairArgs) -> Result<Page<Post>> {
  let fair = refs::fair(&api.ctx.post_ref(&args.fair)?)?;
  let filter = resolve(api, args.words, args.filters).await?;
  args
    .page
    .collect_dated(async |req| jobs::fair(api, &fair, &filter, &req).await)
    .await
}
