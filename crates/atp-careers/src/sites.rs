//! Career sites: the companies ATP knows by a short name, and the links of
//! sites whose vendor it recognizes.
//!
//! - short names: `bytedance`, `tencent`, `alibaba`, `meituan`, `kuaishou`
//!   (or the companies' Chinese names);
//! - Feishu Hire: `jobs.bytedance.com/campus` and `/experienced`, any
//!   `<tenant>.jobs.feishu.cn[/<website>]`;
//! - Beisen: any `<tenant>.zhiye.com`;
//! - Moka: `app.mokahr.com/[m/]<campus-recruitment|social-recruitment|…>/<org>/<siteId>`,
//!   and its short links `app.mokahr.com/su/<code>`;
//! - the big companies' own hosts: `join.qq.com`, `careers.tencent.com`,
//!   `campus-talent.alibaba.com`, `talent.alibaba.com`,
//!   `zhaopin.meituan.com`, `campus.kuaishou.cn`;
//! - any other link: its page is read and recognized by what each
//!   vendor's pages carry (Moka's `init-data`, Feishu Hire's
//!   `js-websiteInfo`, Beisen's `BSGlobal`), as with companies running a
//!   vendor's site on their own domain (campus.didiglobal.com is Moka).

use atp_core::{Collection, Error, Result, json};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::api::{Api, Html, cache_key};
use crate::jobs::Kind;

/// How long what a link was found to be is kept.
const DAY: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// Who runs a site, which decides how it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
  Feishu,
  Tencent,
  Alibaba,
  Meituan,
  Kuaishou,
  Beisen,
  Moka,
}

impl Vendor {
  pub fn name(self) -> &'static str {
    match self {
      Self::Feishu => "feishu",
      Self::Tencent => "tencent",
      Self::Alibaba => "alibaba",
      Self::Meituan => "meituan",
      Self::Kuaishou => "kuaishou",
      Self::Beisen => "beisen",
      Self::Moka => "moka",
    }
  }
}

/// One career site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
  pub vendor: Vendor,
  /// `https://host`.
  pub origin: String,
  /// The path of one website of the origin, without slashes at its ends:
  /// a Feishu Hire website (`campus`, `graduate`), a Moka site
  /// (`campus-recruitment/zhongan/166735`); empty for the others.
  pub path: String,
  /// The kind of jobs the link stands for, where it says (a site for
  /// experienced hires, Beisen's `/campus/…`).
  pub kind: Option<Kind>,
}

impl Site {
  fn new(vendor: Vendor, origin: &str, path: &str, kind: Option<Kind>) -> Self {
    Self {
      vendor,
      origin: origin.trim_end_matches('/').to_owned(),
      path: path.trim_matches('/').to_owned(),
      kind,
    }
  }

  /// The site's own page.
  pub fn url(&self) -> String {
    if self.path.is_empty() {
      self.origin.clone()
    } else {
      format!("{}/{}", self.origin, self.path)
    }
  }

  pub fn host(&self) -> &str {
    self.origin.trim_start_matches("https://")
  }
}

/// A company ATP knows by a short name.
pub struct Builtin {
  pub name: &'static str,
  pub aliases: &'static [&'static str],
  pub company: &'static str,
  pub vendor: Vendor,
  pub origin: &'static str,
  pub path: &'static str,
  pub about: &'static str,
}

pub const BUILTINS: &[Builtin] = &[
  Builtin {
    name: "bytedance",
    aliases: &["字节跳动", "字节"],
    company: "字节跳动",
    vendor: Vendor::Feishu,
    origin: "https://jobs.bytedance.com",
    path: "campus",
    about: "campus jobs and internships (jobs.bytedance.com/campus, on Feishu Hire); --kind social: experienced hires (/experienced)",
  },
  Builtin {
    name: "tencent",
    aliases: &["腾讯"],
    company: "腾讯",
    vendor: Vendor::Tencent,
    origin: "https://join.qq.com",
    path: "",
    about: "campus jobs and internships (join.qq.com) by project; --kind social: experienced hires (careers.tencent.com)",
  },
  Builtin {
    name: "alibaba",
    aliases: &["阿里巴巴", "阿里"],
    company: "阿里巴巴",
    vendor: Vendor::Alibaba,
    origin: "https://campus-talent.alibaba.com",
    path: "",
    about: "campus jobs and internships (campus-talent.alibaba.com) by batch; --kind social: experienced hires (talent.alibaba.com)",
  },
  Builtin {
    name: "meituan",
    aliases: &["美团"],
    company: "美团",
    vendor: Vendor::Meituan,
    origin: "https://zhaopin.meituan.com",
    path: "",
    about: "campus jobs and internships (zhaopin.meituan.com); --kind social: experienced hires",
  },
  Builtin {
    name: "kuaishou",
    aliases: &["快手"],
    company: "快手",
    vendor: Vendor::Kuaishou,
    origin: "https://campus.kuaishou.cn",
    path: "",
    about: "campus jobs and internships (campus.kuaishou.cn) by project",
  },
];

/// The vendors whose sites ATP reads from a link, and the links.
const VENDORS: &[(&str, &str, &str)] = &[
  (
    "feishu",
    "Feishu Hire (飞书招聘)",
    "https://<tenant>.jobs.feishu.cn/<website>, and jobs.bytedance.com",
  ),
  (
    "beisen",
    "Beisen (北森)",
    "https://<tenant>.zhiye.com, its /campus, /intern and /social pages",
  ),
  (
    "moka",
    "Moka",
    "https://app.mokahr.com/campus-recruitment/<org>/<siteId> (or social-recruitment), short links app.mokahr.com/su/…, and Moka sites on companies' own domains",
  ),
];

/// The company of a built-in site, when the site is one.
pub fn company(site: &Site) -> Option<&'static str> {
  BUILTINS
    .iter()
    .find(|b| b.origin == site.origin)
    .map(|b| b.company)
}

/// `sites`: the short names, then the vendors.
pub fn listing() -> Vec<Collection> {
  let mut list: Vec<Collection> = BUILTINS
    .iter()
    .map(|b| Collection {
      id: b.name.into(),
      kind: "site".into(),
      name: b.company.into(),
      description: Some(b.about.into()),
      url: Some(Site::new(b.vendor, b.origin, b.path, None).url()),
      extra: [("vendor".to_owned(), json!(b.vendor.name()))]
        .into_iter()
        .collect(),
      ..Collection::default()
    })
    .collect();
  list.extend(VENDORS.iter().map(|(id, name, links)| Collection {
    id: (*id).into(),
    kind: "vendor".into(),
    name: (*name).into(),
    description: Some(format!("any site by its link: {links}")),
    ..Collection::default()
  }));
  list
}

/// What an argument names before anything is read.
enum Named {
  Site(Site),
  /// A link only its page tells about (or a short link).
  Link(Url),
}

/// A link, with `https://` when it was left out.
pub fn link(arg: &str) -> Option<Url> {
  let arg = arg.trim();
  let url = Url::parse(arg).ok().filter(|u| u.has_host()).or_else(|| {
    (arg.contains('.') && !arg.contains(char::is_whitespace))
      .then(|| Url::parse(&format!("https://{arg}")).ok())
      .flatten()
  })?;
  matches!(url.scheme(), "http" | "https").then_some(url)
}

fn named(arg: &str) -> Result<Named> {
  let short = arg.trim().to_lowercase();
  if let Some(b) = BUILTINS
    .iter()
    .find(|b| b.name == short || b.aliases.contains(&short.as_str()))
  {
    return Ok(Named::Site(Site::new(b.vendor, b.origin, b.path, None)));
  }
  let url = link(arg).ok_or_else(|| {
    Error::input(format!(
      "not a career site: {arg} (expected bytedance, tencent, alibaba, meituan, kuaishou or a site's link)"
    ))
    .with_hint("`atp careers sites` lists them")
  })?;
  Ok(match known(&url) {
    Some(site) => Named::Site(site),
    None => Named::Link(url),
  })
}

/// The segments of a link's path.
fn segments(url: &Url) -> Vec<&str> {
  url
    .path_segments()
    .map(|s| s.filter(|p| !p.is_empty()).collect())
    .unwrap_or_default()
}

/// A site its link names by host and path alone.
pub fn known(url: &Url) -> Option<Site> {
  let host = url.host_str()?.to_ascii_lowercase();
  let origin = format!("https://{host}");
  let parts = segments(url);
  let first = parts.first().copied().unwrap_or_default();
  let site = |vendor, path: &str, kind| Some(Site::new(vendor, &origin, path, kind));
  match host.as_str() {
    "jobs.bytedance.com" if first == "experienced" => {
      site(Vendor::Feishu, "experienced", Some(Kind::Social))
    }
    "jobs.bytedance.com" => site(Vendor::Feishu, "campus", None),
    "join.qq.com" => site(Vendor::Tencent, "", None),
    "careers.tencent.com" => site(Vendor::Tencent, "", Some(Kind::Social)),
    "campus-talent.alibaba.com" => site(Vendor::Alibaba, "", None),
    "talent.alibaba.com" => site(Vendor::Alibaba, "", Some(Kind::Social)),
    "zhaopin.meituan.com" => {
      let social = url.path().contains("social");
      site(Vendor::Meituan, "", social.then_some(Kind::Social))
    }
    "campus.kuaishou.cn" => site(Vendor::Kuaishou, "", None),
    "app.mokahr.com" => moka(url, &origin),
    _ if host.ends_with(".jobs.feishu.cn") => {
      let path = if first == "position" { "" } else { first };
      site(Vendor::Feishu, path, None)
    }
    _ if host.ends_with(".zhiye.com") => site(Vendor::Beisen, "", Kind::from_path(first)),
    _ => None,
  }
}

/// A Moka site: `[m/]<kind of site>/<org>/<siteId>`.
fn moka(url: &Url, origin: &str) -> Option<Site> {
  let parts = segments(url);
  let parts = match parts.first() {
    Some(&"m") => &parts[1..],
    _ => &parts[..],
  };
  let [prefix, org, id, ..] = parts else {
    return None;
  };
  if !id.bytes().all(|b| b.is_ascii_digit()) || *prefix == "su" {
    return None;
  }
  let kind = prefix.starts_with("social").then_some(Kind::Social);
  Some(Site::new(
    Vendor::Moka,
    origin,
    &format!("{prefix}/{org}/{id}"),
    kind,
  ))
}

/// The site an argument names: a short name or a link, whose page is read
/// when its host alone does not tell.
pub async fn resolve(api: &Api, arg: &str) -> Result<Site> {
  match named(arg)? {
    Named::Site(site) => Ok(site),
    Named::Link(url) => recognize(api, &url).await,
  }
}

/// The site `--site` names.
pub async fn current(api: &Api) -> Result<Site> {
  resolve(api, &api.site_arg()?).await
}

/// A site by its page: where a short link leads, or what the page carries.
/// What a link was found to be is kept a day.
async fn recognize(api: &Api, url: &Url) -> Result<Site> {
  let mut link = url.clone();
  link.set_fragment(None);
  let key = cache_key("site", link.as_str());
  if let Some(site) = api.ctx.store.cache_get::<Site>(&key, DAY) {
    return Ok(site);
  }
  let page = api.page(url.as_str()).await?;
  let landed = Url::parse(&page.url).unwrap_or_else(|_| url.clone());
  let site = known(&landed).or_else(|| by_page(&landed, &page));
  let Some(site) = site else {
    return Err(Error::input(format!(
      "ATP does not recognize the career site at {url}: it reads Feishu Hire, Beisen and Moka sites and the built-in companies"
    ))
    .with_hint("`atp careers sites` lists them"));
  };
  api.remember(&site.url(), &page);
  api.ctx.store.cache_put(&key, &site);
  Ok(site)
}

/// A vendor's site on a domain of its own, by what its page carries.
fn by_page(url: &Url, page: &Html) -> Option<Site> {
  let origin = url.origin().ascii_serialization();
  let parts = segments(url);
  let text = &page.text;
  if text.contains("id=\"init-data\"") && text.contains("aesIv") {
    let path = match parts.first() {
      Some(&"m") => parts[1..].join("/"),
      _ => parts.join("/"),
    };
    let kind = path.starts_with("social").then_some(Kind::Social);
    return Some(Site::new(Vendor::Moka, &origin, &path, kind));
  }
  if text.contains("js-websiteInfo") {
    let first = parts.first().copied().unwrap_or_default();
    return Some(Site::new(Vendor::Feishu, &origin, first, None));
  }
  if text.contains("var BSGlobal") {
    let first = parts.first().copied().unwrap_or_default();
    return Some(Site::new(
      Vendor::Beisen,
      &origin,
      "",
      Kind::from_path(first),
    ));
  }
  None
}
