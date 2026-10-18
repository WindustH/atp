//! Yingjiesheng commands beyond the shared set: the talk list with its
//! filters and the codes they take.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Post, Result};

use crate::api::Api;
use crate::filters;
use crate::talks::{self, Filter};

/// What `talks` lists.
#[derive(Debug, clap::Args)]
pub struct TalkArgs {
  /// Words in a company's or school's name or the venue; every talk without them
  pub words: Vec<String>,
  /// Province or region: a name (浙江, 广东 …) or its code from `provinces`
  #[arg(short, long)]
  pub province: Option<String>,
  /// City, by the site's number (`city_id` of a listed talk)
  #[arg(short, long, value_name = "NUMBER")]
  pub city: Option<String>,
  /// School, by the site's number (from `schools`, or `school_id` of a listed talk)
  #[arg(short, long, value_name = "NUMBER")]
  pub school: Option<String>,
  /// Company: the site's number or the link of its talks (the `author` of a talk `read` shows)
  #[arg(long)]
  pub company: Option<String>,
  /// One day only: 2026-10-22, today or tomorrow
  #[arg(short, long)]
  pub date: Option<String>,
  /// Industry: a name or part of one (互联网, 银行 …) or its code from `industries`
  #[arg(short, long)]
  pub industry: Option<String>,
  #[command(flatten)]
  pub page: PageArgs,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Campus recruitment talks (宣讲会) by province, city, school, company, day and industry
  #[command(after_long_help = EX_TALKS)]
  Talks(Box<TalkArgs>),
  /// Provinces and regions with their codes, for talks --province
  Provinces,
  /// Industries with their codes, for talks --industry
  Industries,
  /// The hot schools of a province with their numbers, for talks --school
  Schools {
    /// Province or region: a name (浙江) or its code
    province: String,
  },
}

const EX_TALKS: &str = "\
Examples:
  atp yingjiesheng talks --province 浙江 --date tomorrow
  atp yingjiesheng schools 北京
  atp yingjiesheng talks --school 7 -n 50
  atp yingjiesheng talks 国网 --industry 能源
  atp yingjiesheng read '#1'

Days are 2026-10-22, today or tomorrow; cities and schools are the site's numbers.
More: atp guide yingjiesheng talks";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Talks(args) => Data::Posts(list(api, *args).await?),
    Command::Provinces => Data::Collections(Page::last(filters::provinces())),
    Command::Industries => Data::Collections(Page::last(filters::industries())),
    Command::Schools { province } => {
      Data::Collections(Page::last(filters::schools(api, &province).await?))
    }
  })
}

/// `talks`: the filters as the list takes them.
async fn list(api: &Api, args: TalkArgs) -> Result<Page<Post>> {
  let filter = Filter {
    words: args.words.join(" "),
    province: args
      .province
      .as_deref()
      .map(filters::province)
      .transpose()?,
    city: args
      .city
      .as_deref()
      .map(|c| filters::code(c, "city"))
      .transpose()?,
    school: args
      .school
      .as_deref()
      .map(|s| filters::code(s, "school"))
      .transpose()?,
    company: args
      .company
      .as_deref()
      .map(crate::refs::company)
      .transpose()?,
    date: args.date.as_deref().map(filters::date).transpose()?,
    industry: args
      .industry
      .as_deref()
      .map(filters::industry)
      .transpose()?,
  };
  args
    .page
    .collect(async |req| talks::list(api, &filter, &req).await)
    .await
}
