//! Proginn commands beyond the shared set: the job search with the
//! board's filters, its cities and roles, and applying to one job.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::apply;
use crate::filters::{self, WorkType};
use crate::jobs::{self, Filter};

/// What `jobs` lists.
#[derive(Debug, clap::Args)]
pub struct JobArgs {
  /// Search words (a title, a skill); every open job without them
  pub words: Vec<String>,
  /// Remote work, or on-site work at the client's (驻场)
  #[arg(short = 't', long = "type", value_enum)]
  pub work_type: Option<WorkType>,
  /// City: a name (北京, 杭州 …), the word of the board's links (beijing) or the id `cities` lists
  #[arg(short, long)]
  pub city: Option<String>,
  /// Role or direction: a name `roles` lists (后端, Rust, UI设计师 …), its link word or its id
  #[arg(short, long)]
  pub role: Option<String>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Search jobs with the board's filters: remote or on-site, city, role
  #[command(after_long_help = EX_JOBS)]
  Jobs(JobArgs),
  /// Cities for `jobs --city`: the hot ones, or those matching a name
  #[command(after_long_help = EX_CITIES)]
  Cities {
    /// A city's or a province's name, or the word of the board's links
    name: Option<String>,
  },
  /// Roles (职业角色) and their directions, for `jobs --role`
  Roles,
  /// Apply to one job (申请接单; needs a login)
  #[command(after_long_help = EX_APPLY)]
  Apply {
    /// Job: a job.proginn.com/d/ link, the hash in it, the job's number or #N from the last list
    job: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

const EX_JOBS: &str = "\
Examples:
  atp proginn jobs rust --type remote
  atp proginn jobs --city 上海 --type onsite
  atp proginn jobs --role 后端 -n 60
  atp proginn jobs --role Python --since 7d

Cities and roles are named as the board shows them; `cities` and `roles`
list them.
More: atp guide proginn jobs";

const EX_CITIES: &str = "\
Examples:
  atp proginn cities
  atp proginn cities 苏州
  atp proginn cities 广东

More: atp guide proginn cities";

const EX_APPLY: &str = "\
Examples:
  atp proginn apply https://job.proginn.com/d/5af3107aa141
  atp proginn jobs python --type remote
  atp proginn apply #3

Shows the job and asks before sending; -y skips the question.
More: atp guide proginn apply";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Jobs(args) => Data::Posts(search(api, args).await?),
    Command::Cities { name } => {
      Data::Collections(Page::last(filters::cities(api, name.as_deref()).await?))
    }
    Command::Roles => Data::Collections(Page::last(filters::roles(api).await?)),
    Command::Apply { job, yes } => {
      let job = api.ctx.post_ref(&job)?;
      Data::Action(apply::apply(api, &job, yes).await?)
    }
  })
}

/// `jobs`: the filters resolved to the board's ids.
async fn search(api: &Api, args: JobArgs) -> Result<Page<Post>> {
  let mut filter = Filter {
    keyword: args.words.join(" ").trim().to_owned(),
    work_type: args.work_type,
    ..Filter::default()
  };
  if let Some(city) = &args.city {
    filter.city = Some(filters::city(api, city).await?);
  }
  if let Some(role) = &args.role {
    filter.role = Some(filters::role(api, role).await?);
  }
  args
    .page
    .collect_dated(async |req| jobs::list(api, &filter, &req).await)
    .await
}
