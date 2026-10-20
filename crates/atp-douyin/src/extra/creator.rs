//! The creator platform beyond publishing: your works with their numbers,
//! your drafts, your collections (合集) and the works in them, and what
//! the publish form offers a post (locations, hot spots, official
//! activities).

use std::path::PathBuf;

use atp_core::cli::PageArgs;
use atp_core::{Data, Result};

use crate::api::Api;
use crate::mixes::{self, MixSpec};
use crate::{drafts, insights, places, topics};

#[derive(Debug, clap::Subcommand)]
pub enum CreatorCommand {
  /// Your works with their creator numbers, review state and visibility
  #[command(after_long_help = EX_WORKS)]
  Works {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Your drafts (草稿) on the creator platform
  Drafts {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Find locations (POI, 位置) a post can be tagged with
  #[command(after_long_help = EX_POI)]
  Poi {
    /// Place name or address
    keyword: String,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Hot spots (热点) a post can be linked to
  #[command(after_long_help = EX_HOTSPOTS)]
  Hotspots {
    /// Words to look for; Douyin's recommendations when left out
    words: Option<String>,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Official activities (官方活动) open to your account
  #[command(after_long_help = EX_ACTIVITIES)]
  Activities {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Create a collection (合集), with the works it starts with
  #[command(after_long_help = EX_MIX_CREATE)]
  MixCreate {
    /// Title, 2 to 20 characters
    title: String,
    /// Your works it starts with, in episode order: aweme id, link or #N
    #[arg(value_name = "WORK")]
    works: Vec<String>,
    /// Cover image: JPG or PNG, 5 MB at most, square shows best (the last
    /// work's cover when left out)
    #[arg(long, value_name = "FILE")]
    cover: Option<PathBuf>,
    /// Description, 5 to 200 characters
    #[arg(long, value_name = "TEXT")]
    desc: Option<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Change a collection's (合集) title, description or cover
  #[command(after_long_help = EX_MIX_EDIT)]
  MixEdit {
    /// Collection: id, douyin.com/collection/… link, exact title or #N
    mix: String,
    /// New title, 2 to 20 characters
    #[arg(long)]
    title: Option<String>,
    /// New description, 5 to 200 characters
    #[arg(long, value_name = "TEXT")]
    desc: Option<String>,
    /// New cover image: JPG or PNG, 5 MB at most, square shows best
    #[arg(long, value_name = "FILE")]
    cover: Option<PathBuf>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Delete a collection (合集); its works stay
  #[command(after_long_help = EX_MIX_DELETE)]
  MixDelete {
    /// Collection: id, douyin.com/collection/… link, exact title or #N
    mix: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Add your published works to a collection (合集) as its next episodes
  #[command(after_long_help = EX_MIX_ADD)]
  MixAdd {
    /// Collection: id, douyin.com/collection/… link, exact title or #N
    mix: String,
    /// Works in episode order: aweme id, link or #N (100 at most)
    #[arg(required = true, value_name = "WORK")]
    works: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Take works out of a collection (合集); they stay on your account
  #[command(after_long_help = EX_MIX_REMOVE)]
  MixRemove {
    /// Collection: id, douyin.com/collection/… link, exact title or #N
    mix: String,
    /// Works: aweme id, link or #N
    #[arg(required = true, value_name = "WORK")]
    works: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Reorder a collection's (合集) episodes: the works given first, the others after
  #[command(after_long_help = EX_MIX_SORT)]
  MixSort {
    /// Collection: id, douyin.com/collection/… link, exact title or #N
    mix: String,
    /// Works in their new order: aweme id, link or #N
    #[arg(required = true, value_name = "WORK")]
    works: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

pub async fn run(api: &Api, command: CreatorCommand) -> Result<Data> {
  use CreatorCommand as C;
  Ok(match command {
    C::Works { page } => Data::Posts(
      page
        .collect_dated(async |r| insights::works(api, &r).await)
        .await?,
    ),
    C::Drafts { page } => Data::Posts(
      page
        .collect_dated(async |r| drafts::list(api, &r).await)
        .await?,
    ),
    C::Poi { keyword, page } => Data::Collections(
      page
        .collect(async |r| places::search(api, &keyword, &r).await)
        .await?,
    ),
    C::Hotspots { words, page } => Data::Posts(
      page
        .collect(async |_| topics::hotspots(api, words.as_deref()).await)
        .await?,
    ),
    C::Activities { page } => Data::Collections(
      page
        .collect(async |_| topics::activities(api).await)
        .await?,
    ),
    C::MixCreate {
      title,
      works,
      cover,
      desc,
      yes,
    } => {
      let spec = MixSpec {
        title: Some(title),
        desc,
        cover,
      };
      Data::Action(mixes::create(api, &spec, &works, yes).await?)
    }
    C::MixEdit {
      mix,
      title,
      desc,
      cover,
      yes,
    } => {
      let mix = api.ctx.collection_ref(&mix)?;
      let spec = MixSpec { title, desc, cover };
      Data::Action(mixes::edit(api, &mix, &spec, yes).await?)
    }
    C::MixDelete { mix, yes } => {
      let mix = api.ctx.collection_ref(&mix)?;
      Data::Action(mixes::delete(api, &mix, yes).await?)
    }
    C::MixAdd { mix, works, yes } => {
      let mix = api.ctx.collection_ref(&mix)?;
      Data::Action(mixes::add(api, &mix, &works, yes).await?)
    }
    C::MixRemove { mix, works, yes } => {
      let mix = api.ctx.collection_ref(&mix)?;
      Data::Action(mixes::remove(api, &mix, &works, yes).await?)
    }
    C::MixSort { mix, works, yes } => {
      let mix = api.ctx.collection_ref(&mix)?;
      Data::Action(mixes::sort(api, &mix, &works, yes).await?)
    }
  })
}

const EX_WORKS: &str = "\
Examples:
  atp douyin works -n 100 -f csv > works.csv
  atp douyin insights '#1'            # the numbers of one of them

Rows carry plays, likes, comments, shares and saves in `metrics`, the
other numbers (completion rate, 2 s bounce rate, cover click rate ...),
`review` and `visibility` in `extra`.
More: atp guide douyin works";

const EX_POI: &str = "\
Examples:
  atp douyin poi 三里屯
  atp douyin poi '上海 外滩' -n 30
  atp douyin post --video v.mp4 --title 标题 --location '#1'   # tag a post with the first

More: atp guide douyin poi";

const EX_ACTIVITIES: &str = "\
Examples:
  atp douyin activities
  atp douyin post --video v.mp4 --title 标题 --activity '#1'   # join the first

More: atp guide douyin activities";

const EX_HOTSPOTS: &str = "\
Examples:
  atp douyin hotspots                 # recommended, then all
  atp douyin hotspots 春节

More: atp guide douyin hotspots";

const EX_MIX_CREATE: &str = "\
Examples:
  atp douyin mix-create 操作系统 7697000000000000001 7697000000000000002 7697000000000000003
  atp douyin works                    # then give works by #N, in episode order
  atp douyin mix-create 操作系统 '#3' '#2' '#1' --cover os.jpg --desc '从硬件到进程的操作系统课'

Uses none of the works' 5 edits. A work is in one collection at most and
must be public.
More: atp guide douyin mix-create";

const EX_MIX_EDIT: &str = "\
Examples:
  atp douyin mix-edit 操作系统 --title '操作系统 2026'
  atp douyin mix-edit 7123456789012345678 --cover os.jpg --desc '从硬件到进程的操作系统课'

More: atp guide douyin mix-edit";

const EX_MIX_DELETE: &str = "\
Examples:
  atp douyin mix-delete 操作系统
  atp douyin mix-delete https://www.douyin.com/collection/7123456789012345678 -y

More: atp guide douyin mix-delete";

const EX_MIX_ADD: &str = "\
Examples:
  atp douyin mix-add 操作系统 7697000000000000004 7697000000000000005
  atp douyin works                    # then give works by #N, in episode order
  atp douyin mix-add 操作系统 '#2' '#1'

Uses none of the works' 5 edits. A work is in one collection at most and
must be public.
More: atp guide douyin mix-add";

const EX_MIX_REMOVE: &str = "\
Examples:
  atp douyin mix-remove 操作系统 7697000000000000002

More: atp guide douyin mix-remove";

const EX_MIX_SORT: &str = "\
Examples:
  atp douyin mix-sort 操作系统 7697000000000000003      # now episode 1
  atp douyin mix-sort 操作系统 7697000000000000001 7697000000000000002 7697000000000000003

More: atp guide douyin mix-sort";
