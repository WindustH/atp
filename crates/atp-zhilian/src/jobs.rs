//! Jobs: the search of www's 搜索 page (`fe-api …/search/positions`, 20 a
//! page), with the mobile search page (`m.zhaopin.com/sou/jl<city>/kw<words>/p<n>`,
//! 15 a page, words and city only) when the search asks for a check, and a
//! job's mobile page (`m.zhaopin.com/jobs/<number>.htm`).

use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt, json};

use crate::api::{Api, M, WWW};
use crate::filters::{JobType, NATIONWIDE};
use crate::{parse, refs};

/// Cards per page of the search.
const PAGE: u64 = 20;

/// What a search is narrowed to, in the site's codes.
#[derive(Debug, Clone)]
pub struct Filter {
  pub words: String,
  pub city: String,
  pub salary: Option<String>,
  pub experience: Option<String>,
  pub degree: Option<String>,
  pub job_type: Option<JobType>,
}

impl Default for Filter {
  fn default() -> Self {
    Self {
      words: String::new(),
      city: NATIONWIDE.into(),
      salary: None,
      experience: None,
      degree: None,
      job_type: None,
    }
  }
}

impl Filter {
  fn narrowed(&self) -> bool {
    self.salary.is_some()
      || self.experience.is_some()
      || self.degree.is_some()
      || self.job_type.is_some()
  }
}

/// `search WORDS`: all of China; `--filter` keeps one kind of job.
pub fn search_filter(query: &Query) -> Filter {
  Filter {
    words: query.keyword.clone(),
    job_type: query.filter.as_deref().and_then(JobType::from_name),
    ..Filter::default()
  }
}

/// The search body as www's page sends it.
fn body(filter: &Filter, page: u64) -> Value {
  let mut body = json!({
    "S_SOU_FULL_INDEX": filter.words,
    "S_SOU_WORK_CITY": filter.city,
    "order": 4,
    "pageIndex": page,
    "pageSize": PAGE,
    "anonymous": 1,
    "eventScenario": "pcSearchedSouSearch",
    "platform": 13,
    "version": "0.0.0",
  });
  for (key, value) in [
    ("S_SOU_SALARY", filter.salary.as_deref()),
    ("S_SOU_WORK_EXPERIENCE", filter.experience.as_deref()),
    ("S_SOU_EDUCATION_LOWESTLEVEL", filter.degree.as_deref()),
    ("S_SOU_POSITION_TYPE", filter.job_type.map(JobType::code)),
  ] {
    if let Some(value) = value {
      body[key] = json!(value);
    }
  }
  body
}

/// Search words as the mobile search's links write them: the UTF-16 code
/// units as 16 bits each, padded to fives, in base 32 (`0`-`9`, `A`-`V`).
fn words_path(words: &str) -> String {
  const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
  let mut bits: Vec<bool> = words
    .encode_utf16()
    .flat_map(|unit| (0..16).rev().map(move |i| (unit >> i) & 1 == 1))
    .collect();
  while !bits.len().is_multiple_of(5) {
    bits.push(false);
  }
  bits
    .chunks(5)
    .map(|five| {
      let n = five.iter().fold(0, |n, &bit| n << 1 | usize::from(bit));
      DIGITS[n] as char
    })
    .collect()
}

/// www's search page of the words and city, as the referer and the place
/// of the check.
fn search_page(filter: &Filter) -> String {
  if filter.words.is_empty() {
    format!("{WWW}/sou/jl{}", filter.city)
  } else {
    format!(
      "{WWW}/sou/jl{}/kw{}",
      filter.city,
      words_path(&filter.words)
    )
  }
}

/// One page of the search; the mobile search page when the search wants a
/// check and no filter beyond the words and city is asked for.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  let page = req.number_or(1);
  let referer = search_page(filter);
  let v = api.search(&body(filter, page), &referer).await?;
  if v.i64("isVerification") == Some(1) {
    if filter.narrowed() {
      return Err(
        Error::verification("Zhilian's search wants a check before answering")
          .with_url(referer)
          .unclearable(),
      );
    }
    return mobile(api, filter, page).await;
  }
  let items: Vec<Post> = v.list("list").iter().filter_map(parse::card).collect();
  let more = v.i64("isEndPage") != Some(1) && !items.is_empty();
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// The data a server-rendered page was built from (`window.NAME = {…}` or
/// `NAME={…}`).
pub fn state(html: &str, name: &str) -> Option<Value> {
  let start = html.find(name)?;
  let rest = &html[start + name.len()..];
  let json = rest[rest.find('=')? + 1..].trim_start();
  serde_json::Deserializer::from_str(json)
    .into_iter::<Value>()
    .next()?
    .ok()
}

/// One page of the mobile search page: 15 cards a page.
async fn mobile(api: &Api, filter: &Filter, page: u64) -> Result<Page<Post>> {
  let mut url = format!("{M}/sou/jl{}", filter.city);
  if !filter.words.is_empty() {
    url.push_str(&format!("/kw{}", words_path(&filter.words)));
  }
  if page > 1 {
    url.push_str(&format!("/p{page}"));
  }
  let html = api.page(&url).await?;
  let data = state(&html, "__INITIAL_DATA__")
    .ok_or_else(|| Error::upstream("Zhilian's search page came without its data"))?;
  let items: Vec<Post> = data
    .list("positionList")
    .iter()
    .filter_map(parse::card)
    .collect();
  let ended = data.bool("positionSearchParams.isEndPage") == Some(true);
  let more = !ended && !items.is_empty();
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// A job's mobile page: its `positionDetail`.
pub async fn page(api: &Api, arg: &str) -> Result<(String, Value)> {
  let number = refs::job(arg)?;
  let url = refs::job_url(&number);
  let html = api.page(&url).await?;
  let detail = state(&html, "__INITIAL_STATE__")
    .map(|s| s.at("positionDetail").clone())
    .filter(|d| d.at("positionDetail").is_object())
    .ok_or_else(|| {
      Error::not_found(format!(
        "Zhilian shows no job at {url} (it may have been taken down)"
      ))
    })?;
  Ok((number, detail))
}

/// A job's page: the whole description, pay, place and address,
/// experience, degree, openings, skills and benefits, the company.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let (number, detail) = page(api, arg).await?;
  parse::detail(&detail).ok_or_else(|| {
    Error::not_found(format!(
      "Zhilian shows no job at {}",
      refs::job_url(&number)
    ))
  })
}
