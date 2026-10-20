//! 知识星球 commands beyond the shared set: your groups, one group, a
//! group's topics by the app's tabs, and search inside a group.

use atp_core::cli::PageArgs;
use atp_core::{Data, Page, Result};

use crate::api::Api;
use crate::parse::api_time;
use crate::topics::{self, Scope};
use crate::{groups, refs, search};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// The groups (星球) you joined, with their members, topics and your membership
  #[command(after_long_help = EX_GROUPS)]
  Groups {
    #[command(flatten)]
    page: PageArgs,
  },
  /// One group: its owner, members, topics and your membership, or its public preview
  #[command(after_long_help = EX_GROUP)]
  Group {
    /// Group: wx.zsxq.com/group/<id> link, number or #N after `groups`
    group: String,
  },
  /// A group's topics, newest first, by the app's tabs
  #[command(after_long_help = EX_TOPICS)]
  Topics {
    /// Group: wx.zsxq.com/group/<id> link, number or #N after `groups`
    group: String,
    /// Which tab: every topic, the digests (精华), the owner's or the Q&A
    #[arg(short, long, value_enum, default_value_t = Scope::All)]
    scope: Scope,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Search the topics of one group
  #[command(after_long_help = EX_GROUP_SEARCH)]
  GroupSearch {
    /// Group: wx.zsxq.com/group/<id> link, number or #N after `groups`
    group: String,
    /// Words to search for
    query: String,
    #[command(flatten)]
    page: PageArgs,
  },
}

const EX_GROUPS: &str = "\
Examples:
  atp zsxq groups
  atp zsxq topics '#1' --scope digests
  atp zsxq groups -f json | jq '.data.items[] | {id, name, expires: .extra.expires_at}'

Needs a login. `collections` lists the same groups.
More: atp guide zsxq groups";

const EX_GROUP: &str = "\
Examples:
  atp zsxq group 758548854
  atp zsxq group https://wx.zsxq.com/group/758548854
  atp zsxq group '#2'

Without a login (or for a group you are not in) it is the group's public preview.
More: atp guide zsxq group";

const EX_TOPICS: &str = "\
Examples:
  atp zsxq topics 758548854 -n 50
  atp zsxq topics '#1' --scope digests --since 30d
  atp zsxq topics '#1' --scope questions --until 2026-01-01
  atp zsxq topics 758548854 -f jsonl -n 1000 > topics.jsonl

The cursor is the `end_time` of the last topic. Without a login it lists the
few topics the group's public page shows.
More: atp guide zsxq topics";

const EX_GROUP_SEARCH: &str = "\
Examples:
  atp zsxq group-search 758548854 复盘
  atp zsxq group-search '#1' \"新手 入门\" -n 50

Needs a login and the group's membership; `search` looks outside one group.
More: atp guide zsxq group-search";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Groups { page } => {
      let all = groups::joined(api).await?;
      Data::Collections(page.collect(async |_| Ok(Page::last(all.clone()))).await?)
    }
    Command::Group { group } => {
      let gid = group_arg(api, &group)?;
      Data::Collections(Page::last(vec![groups::one(api, &gid).await?]))
    }
    Command::Topics { group, scope, page } => topics(api, &group, scope, page).await?,
    Command::GroupSearch { group, query, page } => {
      let gid = group_arg(api, &group)?;
      Data::Posts(
        page
          .collect_dated(async |req| search::group(api, &gid, &query, &req).await)
          .await?,
      )
    }
  })
}

fn group_arg(api: &Api, arg: &str) -> Result<String> {
  refs::group(&api.ctx.collection_ref(arg)?)
}

/// `topics`: from `--until` on when no cursor is given; the public
/// preview without a login.
async fn topics(api: &Api, group: &str, scope: Scope, mut page: PageArgs) -> Result<Data> {
  let gid = group_arg(api, group)?;
  if !api.logged_in() {
    return Ok(Data::Posts(topics::preview(api, &gid).await?));
  }
  if page.cursor.is_none()
    && let Some(until) = page.until
  {
    page.cursor = Some(api_time(until));
  }
  Ok(Data::Posts(
    page
      .collect_dated(async |req| topics::list(api, &gid, scope, &req).await)
      .await?,
  ))
}
