//! Moka career sites: `app.mokahr.com/<campus-recruitment|social-recruitment>/<org>/<siteId>`
//! and Moka sites on companies' own domains
//! (`campus.didiglobal.com/campus_apply/didiglobal/96064`), through the
//! requests their pages make:
//!
//! - the page's `<input id="init-data">` holds the site: the company, the
//!   site's id and kind (`mode`), and the IV its answers are encrypted
//!   with (`aesIv`); kept a day;
//! - `POST /api/outer/ats-apply/website/jobs/v2`: the search (words, kinds
//!   of work as `commitments`, offset and limit);
//! - `POST /api/outer/ats-apply/website/job`: a job.
//!
//! Both answer `{data, necromancer}`: `data` is AES-CBC ciphertext
//! (base64, PKCS#7 padding) under the key `necromancer` itself (its UTF-8
//! bytes) and the page's IV, decrypting to `{code: 0, success, data, msg}`.

use std::time::Duration;

use aes::{Aes128, Aes192, Aes256};
use atp_core::{Error, Page, PageReq, Post, Result, Value, ValueExt, json};
use base64::Engine;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use serde::{Deserialize, Serialize};

use crate::api::{self, Api};
use crate::jobs::{self, Filter, Kind};
use crate::parse::moka as parse;
use crate::sites::Site;

/// Jobs per page at most.
const PAGE_MAX: usize = 50;
/// How long a site's description is kept.
const DAY: Duration = Duration::from_secs(24 * 3600);

/// What a site's page says about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
  pub org: String,
  pub site_id: String,
  pub company: String,
  pub iv: String,
  /// `campus` or `social`.
  pub mode: String,
}

impl Info {
  pub fn social(&self) -> bool {
    self.mode.starts_with("social")
  }
}

async fn info(api: &Api, site: &Site) -> Result<Info> {
  let key = api::cache_key("moka", &site.url());
  if let Some(info) = api.ctx.store.cache_get::<Info>(&key, DAY) {
    return Ok(info);
  }
  let page = api.page(&site.url()).await?;
  let info = parse::info(&page.text)
    .ok_or_else(|| Error::upstream(format!("{} is not a Moka site ATP can read", site.url())))?;
  api.ctx.store.cache_put(&key, &info);
  Ok(info)
}

/// Plain text of an encrypted answer.
fn decrypt(data: &str, key: &str, iv: &str) -> Option<Vec<u8>> {
  let mut buf = base64::engine::general_purpose::STANDARD
    .decode(data.trim())
    .ok()?;
  let (key, iv) = (key.as_bytes(), iv.as_bytes());
  let plain = match key.len() {
    16 => cbc::Decryptor::<Aes128>::new_from_slices(key, iv)
      .ok()?
      .decrypt_padded_mut::<Pkcs7>(&mut buf)
      .ok()?,
    24 => cbc::Decryptor::<Aes192>::new_from_slices(key, iv)
      .ok()?
      .decrypt_padded_mut::<Pkcs7>(&mut buf)
      .ok()?,
    32 => cbc::Decryptor::<Aes256>::new_from_slices(key, iv)
      .ok()?
      .decrypt_padded_mut::<Pkcs7>(&mut buf)
      .ok()?,
    _ => return None,
  };
  Some(plain.to_vec())
}

/// A call of the site, its `data` decrypted and checked.
async fn call(api: &Api, site: &Site, info: &Info, path: &str, body: &Value) -> Result<Value> {
  let req = api
    .ctx
    .http
    .post(format!("{}/api/outer/ats-apply/website{path}", site.origin))
    .header("origin", &site.origin)
    .header("referer", site.url())
    .json(body);
  let mut v = api.json(req, &site.url()).await?;
  if let (Some(data), Some(key)) = (v.str("data"), v.str("necromancer")) {
    let plain = decrypt(&data, &key, &info.iv).ok_or_else(|| {
      Error::upstream(format!(
        "{}'s answer did not decrypt with the key and IV it gave",
        site.host()
      ))
    })?;
    v = serde_json::from_slice(&plain)
      .map_err(|_| Error::upstream(format!("{}'s decrypted answer is not JSON", site.host())))?;
  }
  match v.i64("code") {
    Some(0) => Ok(v.at("data").clone()),
    code => Err(api::upstream(
      site.host(),
      v.first_str(&["msg", "message"]),
      code.unwrap_or_default(),
    )),
  }
}

/// `commitments` of a kind on a campus site: internships or the rest.
fn commitments(site: &Site, info: &Info, kind: Option<Kind>) -> Result<Option<&'static str>> {
  let other = |lists: &str| {
    Err(Error::input(format!(
      "{} lists {lists} only; pass the company's other Moka site for the rest",
      site.url()
    )))
  };
  match (info.social(), kind) {
    (_, None) | (true, Some(Kind::Social)) => Ok(None),
    (true, Some(_)) => other("experienced hires"),
    (false, Some(Kind::Social)) => other("campus jobs and internships"),
    (false, Some(Kind::Intern)) => Ok(Some("实习")),
    (false, Some(Kind::Campus)) => Ok(Some("全职")),
  }
}

pub async fn search(api: &Api, site: &Site, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  for (set, what) in [
    (&filter.city, "city"),
    (&filter.batch, "batch"),
    (&filter.category, "category"),
  ] {
    if set.is_some() {
      return Err(jobs::unfiltered(site, what));
    }
  }
  let info = info(api, site).await?;
  let offset = req.number_or(0);
  let mut body = json!({
    "orgId": info.org,
    "siteId": info.site_id.parse::<u64>().unwrap_or_default(),
    "limit": req.size_within(PAGE_MAX),
    "offset": offset,
    "needStat": true,
    "site": if info.social() { "social" } else { "campus" },
    "locale": "zh-CN",
  });
  if !filter.words.is_empty() {
    body["keyword"] = json!(filter.words);
  }
  if let Some(commitment) = commitments(site, &info, filter.kind(site))? {
    body["commitments"] = json!([commitment]);
  }
  let v = call(api, site, &info, "/jobs/v2", &body).await?;
  let rows = v.list("jobs");
  let items: Vec<Post> = rows
    .iter()
    .filter_map(|row| parse::job(row, site, &info))
    .map(|job| job.post(false))
    .collect();
  let seen = offset + rows.len() as u64;
  let more = !rows.is_empty() && seen < v.u64("jobStats.total").unwrap_or(0);
  Ok(Page::new(items, more.then(|| seen.to_string())))
}

pub async fn read(api: &Api, site: &Site, id: &str) -> Result<Post> {
  let info = info(api, site).await?;
  let body = json!({
    "orgId": info.org,
    "siteId": info.site_id.parse::<u64>().unwrap_or_default(),
    "jobId": id,
    "locale": "zh-CN",
  });
  let v = call(api, site, &info, "/job", &body).await?;
  parse::job(&v, site, &info)
    .map(|job| job.post(true))
    .ok_or_else(|| {
      Error::not_found(format!(
        "{} shows no job {id} (it may have been taken down)",
        site.url()
      ))
    })
}
