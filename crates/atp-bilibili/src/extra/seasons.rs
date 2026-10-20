//! Your own collections (合集): listing them, creating, changing and
//! deleting one, and putting videos in, taking them out and ordering
//! them. Reading someone's 合集 is `lists` / `list`; saving one is `save`.

use std::path::PathBuf;

use atp_core::cli::PageArgs;
use atp_core::{Data, Error, Result};

use super::video_ref;
use crate::api::Api;
use crate::refs::{self, Video};
use crate::seasons::{self, Info};

#[derive(Debug, clap::Subcommand)]
pub enum SeasonCommand {
  /// Your 合集 (collections you made): id, title, videos and review state
  #[command(after_long_help = EX_SEASONS)]
  Seasons {
    #[command(flatten)]
    page: PageArgs,
  },
  /// One of your 合集: its videos in order, section by section
  #[command(after_long_help = EX_SEASON)]
  Season {
    /// 合集: id from `seasons`, #N, `MID/ID` or its space link
    season: String,
  },
  /// Create a 合集
  #[command(after_long_help = EX_SEASON_CREATE)]
  SeasonCreate {
    /// Title (at most 50 characters)
    title: String,
    /// Cover image, 16:9 (JPEG or PNG); the upload page's default picture without it
    #[arg(long, value_name = "PATH")]
    cover: Option<PathBuf>,
    /// Description (at most 500 characters)
    #[arg(long, value_name = "TEXT")]
    desc: Option<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Change the title, description or cover of one of your 合集
  #[command(after_long_help = EX_SEASON_EDIT)]
  SeasonEdit {
    /// 合集: id from `seasons`, #N, `MID/ID` or its space link
    season: String,
    /// New title (at most 50 characters)
    #[arg(long)]
    title: Option<String>,
    /// New description; an empty one clears it
    #[arg(long, value_name = "TEXT")]
    desc: Option<String>,
    /// New cover image, 16:9 (JPEG or PNG)
    #[arg(long, value_name = "PATH")]
    cover: Option<PathBuf>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Delete one of your 合集; its videos stay published
  SeasonDelete {
    /// 合集: id from `seasons`, #N, `MID/ID` or its space link
    season: String,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Add your videos to a 合集, in the order given
  #[command(after_long_help = EX_SEASON_ADD)]
  SeasonAdd {
    /// 合集: id from `seasons`, #N, `MID/ID` or its space link
    season: String,
    /// Videos: BV… or av… ids, links, b23.tv short links or #N
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Section (分组 / 小节) to add to, by id; the last one by default
    #[arg(long, value_name = "ID")]
    section: Option<u64>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Take videos out of one of your 合集; they stay published
  SeasonRemove {
    /// 合集: id from `seasons`, #N, `MID/ID` or its space link
    season: String,
    /// Videos: BV… or av… ids, links, b23.tv short links or #N
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Order the videos of a 合集: those given first, in that order
  #[command(after_long_help = EX_SEASON_SORT)]
  SeasonSort {
    /// 合集: id from `seasons`, #N, `MID/ID` or its space link
    season: String,
    /// Videos of one section in their new order: BV… or av… ids, links or #N
    #[arg(required = true, value_name = "VIDEO")]
    videos: Vec<String>,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
}

const EX_SEASONS: &str = "\
Examples:
  atp bili seasons
  atp bili season '#1'

More: atp guide bili seasons";

const EX_SEASON: &str = "\
Examples:
  atp bili season 1234567
  atp bili season '#1' -f json

More: atp guide bili season";

const EX_SEASON_CREATE: &str = "\
Examples:
  atp bili season-create '操作系统' --cover os.jpg --desc '操作系统课程笔记'

More: atp guide bili season-create";

const EX_SEASON_EDIT: &str = "\
Examples:
  atp bili season-edit 1234567 --title '操作系统（完整版）'
  atp bili season-edit 1234567 --cover os-new.jpg --desc ''

More: atp guide bili season-edit";

const EX_SEASON_ADD: &str = "\
Examples:
  atp bili season-add 1234567 BV1xx411c7mD BV1yy411c7mE
  atp bili season-add 1234567 BV1zz411c7mF --section 7654321

More: atp guide bili season-add";

const EX_SEASON_SORT: &str = "\
Examples:
  atp bili season-sort 1234567 BV1yy411c7mE BV1xx411c7mD

More: atp guide bili season-sort";

/// A 合集 argument: `#N` after `seasons`, an id, `season:ID`, `MID/ID`
/// or a space link.
fn season_id(api: &Api, arg: &str) -> Result<u64> {
  let arg = api.ctx.collection_ref(arg)?;
  let id = arg.strip_prefix("season:").unwrap_or(&arg);
  if let Ok(id) = id.parse() {
    return Ok(id);
  }
  match refs::list(&arg) {
    Ok((_, id, "season")) => id
      .parse()
      .map_err(|_| Error::input(format!("bad 合集 id in {arg}"))),
    _ => Err(Error::input(format!(
      "not a 合集: {arg} (expected its id from `atp bili seasons`, #N, MID/ID or its space link)"
    ))),
  }
}

async fn videos(api: &Api, args: &[String]) -> Result<Vec<Video>> {
  let mut out = Vec::with_capacity(args.len());
  for arg in args {
    out.push(video_ref(api, arg).await?);
  }
  Ok(out)
}

pub async fn run(api: &Api, command: SeasonCommand) -> Result<Data> {
  use SeasonCommand as S;
  Ok(match command {
    S::Seasons { page } => {
      Data::Collections(page.collect(async |r| seasons::list(api, &r).await).await?)
    }
    S::Season { season } => Data::Posts(seasons::show(api, season_id(api, &season)?).await?),
    S::SeasonCreate {
      title,
      cover,
      desc,
      yes,
    } => {
      let info = Info {
        title: Some(&title),
        desc: desc.as_deref(),
        cover: cover.as_deref(),
      };
      Data::Action(seasons::create(api, &info, yes).await?)
    }
    S::SeasonEdit {
      season,
      title,
      desc,
      cover,
      yes,
    } => {
      let info = Info {
        title: title.as_deref(),
        desc: desc.as_deref(),
        cover: cover.as_deref(),
      };
      Data::Action(seasons::edit(api, season_id(api, &season)?, &info, yes).await?)
    }
    S::SeasonDelete { season, yes } => {
      Data::Action(seasons::delete(api, season_id(api, &season)?, yes).await?)
    }
    S::SeasonAdd {
      season,
      videos: list,
      section,
      yes,
    } => {
      let id = season_id(api, &season)?;
      let list = videos(api, &list).await?;
      Data::Actions(seasons::add(api, id, &list, section, yes).await?)
    }
    S::SeasonRemove {
      season,
      videos: list,
      yes,
    } => {
      let id = season_id(api, &season)?;
      let list = videos(api, &list).await?;
      Data::Actions(seasons::remove(api, id, &list, yes).await?)
    }
    S::SeasonSort {
      season,
      videos: list,
      yes,
    } => {
      let id = season_id(api, &season)?;
      let list = videos(api, &list).await?;
      Data::Action(seasons::sort(api, id, &list, yes).await?)
    }
  })
}
