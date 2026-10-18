//! Moka's answers: the site its page describes, and jobs (search rows and
//! job pages alike).

use atp_core::{Value, ValueExt};

use super::{Job, job_type, local, text};
use crate::jobs::Kind;
use crate::moka::Info;
use crate::sites::Site;

/// The site of a page: `<input id="init-data" value="{…}">`, its JSON
/// written with HTML entities.
pub fn info(html: &str) -> Option<Info> {
  let start = html.find("id=\"init-data\"")?;
  let tag = &html[html[..start].rfind('<')?..];
  let tag = &tag[..tag.find('>')?];
  let value = &tag[tag.find("value=\"")? + "value=\"".len()..];
  let value = &value[..value.find('"')?];
  let v: Value = serde_json::from_str(&unescape(value)).ok()?;
  Some(Info {
    org: v.first_str(&["org.id", "orgId"])?,
    site_id: v.first_str(&["siteId", "org.siteId"])?,
    company: v
      .first_str(&["org.name", "org.displayName"])
      .unwrap_or_default(),
    iv: v.str("aesIv")?,
    mode: v.str("mode").unwrap_or_else(|| "campus".into()),
  })
}

/// An attribute's text without its entities.
fn unescape(text: &str) -> String {
  text
    .replace("&quot;", "\"")
    .replace("&#34;", "\"")
    .replace("&#39;", "'")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&amp;", "&")
}

/// A place: its city, or the province with a district (上海市·黄浦区,
/// 江苏·新吴区), as Moka names districts rather than their cities.
fn place(v: &Value) -> Option<String> {
  let city = v.str("cityName");
  let province = v.str("provinceName");
  match (city, province) {
    (Some(city), Some(province)) if city.ends_with(['区', '县']) => {
      Some(format!("{province}·{city}"))
    }
    (Some(city), _) => Some(city),
    (None, province) => province.or_else(|| v.str("address")),
  }
}

/// A job, from the search or its page.
pub fn job(v: &Value, site: &Site, info: &Info) -> Option<Job> {
  let id = v.str("id")?;
  let commitment = v.str("commitment");
  let recruit = if info.social() {
    Kind::Social
  } else if commitment.as_deref().is_some_and(|c| c.contains("实习")) {
    Kind::Intern
  } else {
    Kind::Campus
  };
  let mut cities: Vec<String> = v.list("locations").iter().filter_map(place).collect();
  cities.dedup();
  let company = if info.company.is_empty() {
    site.host().to_owned()
  } else {
    info.company.clone()
  };
  Some(Job {
    url: format!("{}#/job/{id}", site.url()),
    title: v.str("title"),
    text: text(v, "jobDescription"),
    company,
    site: site.url(),
    cities,
    recruit: Some(recruit),
    job_type: commitment.as_deref().and_then(job_type),
    category: v.str("zhineng.name"),
    department: v.str("department.name"),
    experience: v.str("experience"),
    degree: v.str("education"),
    created_at: v
      .first_str(&["publishedAt", "openedAt"])
      .as_deref()
      .and_then(local),
    updated_at: v.str("updatedAt").as_deref().and_then(local),
    raw: v.clone(),
    id,
    ..Job::default()
  })
}
