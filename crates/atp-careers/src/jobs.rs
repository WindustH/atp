//! Jobs of a career site: the search with what each backend can narrow it
//! by, job pages, and the codes of a site's filters (batches, cities,
//! categories), handed to the backend of the site's vendor.

use atp_core::{Collection, Error, Page, PageReq, Post, Query, Result};
use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::sites::{self, Site, Vendor};
use crate::{alibaba, beisen, feishu, kuaishou, meituan, moka, refs, tencent};

/// `search --filter` and `jobs --kind`: what kind of hiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
  /// 校招: full-time jobs for new graduates
  #[value(alias = "校招", alias = "graduate")]
  Campus,
  /// 实习: internships
  #[value(alias = "实习")]
  Intern,
  /// 社招: experienced hires
  #[value(alias = "社招", alias = "experienced")]
  Social,
}

impl Kind {
  /// `recruit_type`.
  pub fn name(self) -> &'static str {
    match self {
      Self::Campus => "campus",
      Self::Intern => "intern",
      Self::Social => "social",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    [Self::Campus, Self::Intern, Self::Social]
      .into_iter()
      .find(|kind| kind.name() == name)
  }

  /// The kind a site's path names (`/campus/jobs`, `/social/detail`).
  pub fn from_path(part: &str) -> Option<Self> {
    match part {
      "campus" => Some(Self::Campus),
      "intern" | "internship" => Some(Self::Intern),
      "social" | "experienced" | "society" => Some(Self::Social),
      _ => None,
    }
  }

  /// `job_type`: internships are `intern`, the others full-time.
  pub fn job_type(self) -> &'static str {
    match self {
      Self::Intern => "intern",
      _ => "full-time",
    }
  }
}

/// `search --filter`.
pub const KINDS: &[&str] = &["campus", "intern", "social"];

/// What a search is narrowed to.
#[derive(Debug, Clone, Default)]
pub struct Filter {
  pub words: String,
  pub kind: Option<Kind>,
  /// A city by name, or by the site's code.
  pub city: Option<String>,
  /// A batch or project by the site's id (`batches`).
  pub batch: Option<String>,
  /// A category by the site's id (`categories`).
  pub category: Option<String>,
}

impl Filter {
  /// The kind asked for, or the one the site's link stands for.
  pub fn kind(&self, site: &Site) -> Option<Kind> {
    self.kind.or(site.kind)
  }
}

/// `search WORDS --filter KIND`.
pub fn search_filter(query: &Query) -> Filter {
  Filter {
    words: query.keyword.trim().to_owned(),
    kind: query.filter.as_deref().and_then(Kind::from_name),
    ..Filter::default()
  }
}

/// One page of a site's jobs.
pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  match site.vendor {
    Vendor::Feishu => feishu::search(api, site, filter, req).await,
    Vendor::Tencent => tencent::search(api, site, filter, req).await,
    Vendor::Alibaba => alibaba::search(api, site, filter, req).await,
    Vendor::Meituan => meituan::search(api, site, filter, req).await,
    Vendor::Kuaishou => kuaishou::search(api, site, filter, req).await,
    Vendor::Beisen => beisen::search(api, site, filter, req).await,
    Vendor::Moka => moka::search(api, site, filter, req).await,
  }
}

/// A job's page: a job link of any site, or a job's id on the `--site`.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let job = refs::job(api, arg).await?;
  let site = &job.site;
  match site.vendor {
    Vendor::Feishu => feishu::read(api, site, &job.id).await,
    Vendor::Tencent => tencent::read(api, site, &job.id).await,
    Vendor::Alibaba => alibaba::read(api, site, &job.id).await,
    Vendor::Meituan => meituan::read(api, site, &job.id).await,
    Vendor::Kuaishou => kuaishou::read(api, &job.id).await,
    Vendor::Beisen => beisen::read(api, site, &job.id).await,
    Vendor::Moka => moka::read(api, site, &job.id).await,
  }
}

/// What a site's filter offers, for `batches`, `cities` and `categories`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codes {
  Batches,
  Cities,
  Categories,
}

impl Codes {
  fn name(self) -> &'static str {
    match self {
      Self::Batches => "batches",
      Self::Cities => "cities",
      Self::Categories => "categories",
    }
  }
}

/// The batches, cities or categories of the `--site`'s filters.
pub async fn codes(api: &Api, codes: Codes, kind: Option<Kind>) -> Result<Vec<Collection>> {
  let site = sites::current(api).await?;
  match site.vendor {
    Vendor::Feishu => feishu::codes(api, &site, codes, kind).await,
    Vendor::Tencent => tencent::codes(api, &site, codes, kind).await,
    Vendor::Alibaba => alibaba::codes(api, &site, codes, kind).await,
    Vendor::Kuaishou => kuaishou::codes(api, &site, codes).await,
    Vendor::Meituan | Vendor::Beisen | Vendor::Moka => Err(no_codes(&site, codes)),
  }
}

/// An error for codes a site does not list.
pub fn no_codes(site: &Site, codes: Codes) -> Error {
  Error::input(format!(
    "{} offers no {} to list (`atp guide careers {}` tells what each site has)",
    site.host(),
    codes.name(),
    codes.name()
  ))
}

/// An error for a filter a site does not have.
pub fn unfiltered(site: &Site, what: &str) -> Error {
  Error::input(format!(
    "{} cannot narrow its jobs by {what} (`atp guide careers jobs` tells what each site can)",
    site.host()
  ))
}

/// The code of a name in a site's list (cities, categories): the code
/// itself, the same name or the last part of one (后端 for 研发/后端), or
/// a name starting with it (北京 for 北京市).
pub fn pick<'a>(list: &'a [Collection], wanted: &str) -> Option<&'a Collection> {
  let wanted = wanted.trim();
  let last = format!("/{wanted}");
  list
    .iter()
    .find(|c| c.id == wanted || c.name == wanted)
    .or_else(|| list.iter().find(|c| c.name.ends_with(&last)))
    .or_else(|| list.iter().find(|c| c.name.starts_with(wanted)))
    .or_else(|| list.iter().find(|c| wanted.starts_with(c.name.as_str())))
}

/// `pick`, or an error naming the command that lists the choices.
pub fn choose(list: &[Collection], wanted: &str, site: &Site, what: &str) -> Result<String> {
  pick(list, wanted).map(|c| c.id.clone()).ok_or_else(|| {
    let command = if what == "city" {
      "cities"
    } else {
      "categories"
    };
    Error::input(format!("{} has no {what} {wanted}", site.host()))
      .with_hint(format!("`atp careers {command} --site …` lists them"))
  })
}
