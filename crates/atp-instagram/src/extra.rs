//! Instagram commands beyond the shared set.

use atp_core::cli::{PageArgs, read_text};
use atp_core::{Data, Error, Result};
use atp_meta::Graph;
use atp_meta::graph::FACEBOOK;

use crate::api::Api;
use crate::containers::{self, Compose};
use crate::{relations, stories, topics};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
  /// Posts of a hashtag
  Tag {
    /// Hashtag: `#tag` or `tag`
    tag: String,
    /// Newest instead of top posts
    #[arg(long)]
    recent: bool,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Posts of a place (id or /explore/locations/ link)
  Location {
    /// Place: id, /explore/locations/ link or #N
    location: String,
    /// Newest instead of top posts
    #[arg(long)]
    recent: bool,
    #[command(flatten)]
    page: PageArgs,
  },
  /// Current stories of a user
  Stories {
    /// User: name, @name, profile link, numeric id or #N
    user: String,
  },
  /// Story highlights of a user
  #[command(after_long_help = EX_HIGHLIGHTS)]
  Highlights {
    /// User: name, @name, profile link, numeric id or #N
    user: String,
  },
  /// The stories inside one highlight
  Highlight {
    /// Highlight: id or `highlight:ID` from `highlights`, or #N after it
    highlight: String,
  },
  /// Accounts asking to follow your private account
  #[command(after_long_help = EX_FOLLOW_REQUESTS)]
  FollowRequests {
    #[command(flatten)]
    page: PageArgs,
  },
  /// Accept follow requests: let the users follow you
  AcceptFollower {
    /// Users: names, @names, profile links, numeric ids, or #N from the last user list
    #[arg(required = true, value_name = "USER")]
    users: Vec<String>,
  },
  /// Decline follow requests
  DenyFollower {
    /// Users: names, @names, profile links, numeric ids, or #N from the last user list
    #[arg(required = true, value_name = "USER")]
    users: Vec<String>,
  },
  /// Publish a photo, a carousel or a reel through the Graph API, from public URLs
  #[command(after_long_help = EX_PUBLISH)]
  Publish {
    /// Caption, with its #hashtags and @mentions; `-` reads it from stdin
    caption: Option<String>,
    /// Public URL of a JPEG image; several images or videos make a carousel
    #[arg(long = "image", value_name = "URL")]
    images: Vec<String>,
    /// Public URL of an MP4 or MOV video; one alone is a reel
    #[arg(long = "video", value_name = "URL")]
    videos: Vec<String>,
    /// Public URL of a cover image for the reel
    #[arg(long, value_name = "URL")]
    cover: Option<String>,
    /// Show the reel in the Reels tab only, not on the profile grid
    #[arg(long)]
    reels_only: bool,
    /// Skip the confirmation question (needed in scripts)
    #[arg(short, long)]
    yes: bool,
  },
  /// Marketing API: ad accounts, campaigns, ad sets, ads, insights reports
  #[command(subcommand)]
  Ads(atp_meta::ads::Command),
}

const EX_PUBLISH: &str = "\
Examples:
  atp instagram publish 'Morning light #sea' --image https://example.com/photo.jpg
  atp instagram publish 'Three views' --image https://example.com/a.jpg --image https://example.com/b.jpg --video https://example.com/c.mp4
  atp instagram publish 'Behind the scenes' --video https://example.com/clip.mp4 --cover https://example.com/cover.jpg

Media are public URLs that Meta downloads; local files are refused.
Needs a Graph API token of a business or creator account.
More: atp guide instagram publish";

const EX_HIGHLIGHTS: &str = "\
Examples:
  atp instagram highlights @natgeo
  atp instagram highlight '#2'

More: atp guide instagram highlights";

const EX_FOLLOW_REQUESTS: &str = "\
Examples:
  atp instagram follow-requests
  atp instagram accept-follower '#1' '#3'
  atp instagram deny-follower someone

More: atp guide instagram follow-requests";

pub async fn run(api: &Api, command: Command) -> Result<Data> {
  Ok(match command {
    Command::Tag { tag, recent, page } => Data::Posts(
      page
        .collect_dated(async |req| topics::tag(api, &tag, recent, &req).await)
        .await?,
    ),
    Command::Location {
      location,
      recent,
      page,
    } => {
      let location = api.ctx.collection_ref(&location)?;
      Data::Posts(
        page
          .collect_dated(async |req| topics::location(api, &location, recent, &req).await)
          .await?,
      )
    }
    Command::Stories { user } => {
      Data::Posts(stories::stories(api, &api.ctx.user_ref(&user)?).await?)
    }
    Command::Highlights { user } => {
      Data::Collections(stories::highlights(api, &api.ctx.user_ref(&user)?).await?)
    }
    Command::Highlight { highlight } => {
      let highlight = api.ctx.collection_ref(&highlight)?;
      Data::Posts(stories::highlight(api, &highlight).await?)
    }
    Command::FollowRequests { page } => Data::Users(
      page
        .collect(async |req| relations::requests(api, &req).await)
        .await?,
    ),
    Command::AcceptFollower { users } => {
      Data::actions(relations::answer(api, &user_refs(api, &users)?, true).await?)
    }
    Command::DenyFollower { users } => {
      Data::actions(relations::answer(api, &user_refs(api, &users)?, false).await?)
    }
    Command::Publish {
      caption,
      images,
      videos,
      cover,
      reels_only,
      yes,
    } => {
      let compose = Compose {
        caption: read_text(caption)?,
        images,
        videos,
        cover,
        reels_only,
      };
      Data::Action(containers::publish(api, &compose, yes).await?)
    }
    Command::Ads(command) => atp_meta::ads::run(&api.ctx, &ads_graph(api)?, command).await?,
  })
}

/// User arguments with `#N` resolved.
fn user_refs(api: &Api, users: &[String]) -> Result<Vec<String>> {
  users.iter().map(|u| api.ctx.user_ref(u)).collect()
}

/// The Marketing API: always graph.facebook.com.
fn ads_graph(api: &Api) -> Result<Graph<'_>> {
  let token = api.token().ok_or_else(|| {
    Error::auth("no Graph API access token")
      .with_hint("set META_ACCESS_TOKEN or run `atp instagram login --token <token>`")
  })?;
  Ok(Graph::new(&api.ctx, FACEBOOK, token))
}
