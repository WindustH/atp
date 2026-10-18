//! Jobs: the search of the site's search page (`/app/interns/search/v2`,
//! 20 cards a page): internships with the page's filters, whose cards are
//! written with the private font ([`Glyphs`]), or campus jobs
//! (`intern_type=xz`, words and city only); a job's page
//! (`interns/info/wxz`) and a company's jobs (`company/interns/wxz`, 10 a
//! page).

use atp_core::text::encode_uri_component;
use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt};

use crate::api::{Api, Route, WWW};
use crate::filters::{Degree, Kind, Months, Offer, Pay, Published};
use crate::font::Glyphs;
use crate::parse::{self, Recruit};
use crate::{refs, users};

const SEARCH: &str = "/app/interns/search/v2";
const INFO: &str = "/api/interns/v3.0/interns/info/wxz";
const COMPANY_JOBS: &str = "/api/interns/v3.0/company/interns/wxz";
/// All of China, the search's default city.
pub const NATIONWIDE: &str = "全国";
/// Rows per page of a company's jobs, as its page asks for them.
const COMPANY_PAGE: u64 = 10;

/// What a search is narrowed to.
#[derive(Debug, Clone)]
pub struct Filter {
  pub words: String,
  pub kind: Kind,
  pub city: String,
  pub days: Option<u8>,
  pub months: Option<Months>,
  pub degree: Option<Degree>,
  pub pay: Option<Pay>,
  pub offer: Option<Offer>,
  pub published: Option<Published>,
  /// Well-known companies only (知名企业).
  pub top: bool,
  /// Newest first instead of the site's order.
  pub newest: bool,
}

impl Default for Filter {
  fn default() -> Self {
    Self {
      words: String::new(),
      kind: Kind::Intern,
      city: NATIONWIDE.into(),
      days: None,
      months: None,
      degree: None,
      pay: None,
      offer: None,
      published: None,
      top: false,
      newest: false,
    }
  }
}

impl Filter {
  /// Whether a filter only the internship search has is set.
  pub fn narrowed(&self) -> bool {
    self.days.is_some()
      || self.months.is_some()
      || self.degree.is_some()
      || self.pay.is_some()
      || self.offer.is_some()
      || self.published.is_some()
      || self.top
      || self.newest
  }
}

/// `search WORDS`: internships, or with `--filter campus` campus jobs;
/// `--sort new` puts the newest internships first.
pub fn search_filter(query: &Query) -> Filter {
  Filter {
    words: query.keyword.clone(),
    kind: query
      .filter
      .as_deref()
      .and_then(Kind::from_name)
      .unwrap_or_default(),
    newest: query.sort.as_deref() == Some("new"),
    ..Filter::default()
  }
}

/// The query as the search page sends it: every filter of the internship
/// search, empty when not set; the campus search takes words and city.
fn params(filter: &Filter, page: u64) -> Vec<(&'static str, String)> {
  let mut params = vec![
    ("page", page.to_string()),
    ("keyword", filter.words.clone()),
    ("city", filter.city.clone()),
  ];
  if filter.kind == Kind::Campus {
    params.push(("intern_type", "xz".into()));
    return params;
  }
  let code = |value: Option<&str>| value.unwrap_or_default().to_owned();
  params.extend([
    ("type", "intern".into()),
    ("area", String::new()),
    ("months", code(filter.months.map(Months::code))),
    (
      "days",
      filter.days.map(|d| d.to_string()).unwrap_or_default(),
    ),
    ("degree", code(filter.degree.map(Degree::code))),
    ("official", code(filter.offer.map(Offer::code))),
    (
      "enterprise",
      if filter.top { "china500" } else { "" }.into(),
    ),
    ("salary", filter.pay.map_or("-0", Pay::code).into()),
    ("publishTime", code(filter.published.map(Published::code))),
    ("sortType", if filter.newest { "zj" } else { "" }.into()),
    ("internExtend", String::new()),
  ]);
  params
}

/// The search page of the words, as the referer.
fn search_page(filter: &Filter) -> String {
  let kind = match filter.kind {
    Kind::Intern => "intern",
    Kind::Campus => "school",
  };
  format!(
    "{WWW}/interns?keyword={}&city={}&type={kind}",
    encode_uri_component(&filter.words),
    encode_uri_component(&filter.city)
  )
}

/// One page of the search.
pub async fn list(api: &Api, filter: &Filter, req: &PageReq) -> Result<Page<Post>> {
  if filter.kind == Kind::Campus && filter.narrowed() {
    return Err(Error::input(
      "the campus search takes words and a city only; the other filters and --new are for internships",
    ));
  }
  let page = req.number_or(1);
  let v = api
    .get(
      Route::Site,
      SEARCH,
      &params(filter, page),
      &search_page(filter),
    )
    .await?;
  let msg = v.at("msg");
  let cards = msg.list("data");
  let items: Vec<Post> = match filter.kind {
    Kind::Campus => cards
      .iter()
      .filter_map(|card| parse::card(card, Recruit::Campus))
      .collect(),
    Kind::Intern => decoded(api, cards)
      .await?
      .iter()
      .filter_map(|card| parse::card(card, Recruit::Intern))
      .collect(),
  };
  let size = msg.u64("pageNumber").filter(|n| *n > 0).unwrap_or(20);
  let total = msg.u64("total").unwrap_or(0);
  let more = !cards.is_empty() && page * size < total;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}

/// Whether a card is written with the private font.
fn obfuscated(card: &Value) -> bool {
  card.to_string().contains("&#x")
}

/// Whether a decoded card reads right: no reference left in its title,
/// and its pay as the plain figures the card also carries.
fn fits(card: &Value) -> bool {
  let same = |shown: &str, plain: &str| match (card.str(shown), card.u64(plain)) {
    (Some(text), Some(n)) => text.trim().parse::<u64>().ok() == Some(n),
    _ => true,
  };
  let title = card.str("name").unwrap_or_default();
  !title.contains("&#x") && same("minsal", "minsalary") && same("maxsal", "maxsalary")
}

/// The cards with their private characters decoded. When the cached map
/// does not fit them, the font is read again.
async fn decoded(api: &Api, cards: &[Value]) -> Result<Vec<Value>> {
  if !cards.iter().any(obfuscated) {
    return Ok(cards.to_vec());
  }
  let glyphs = Glyphs::load(api, false).await?;
  let decoded: Vec<Value> = cards.iter().map(|c| glyphs.decode_value(c)).collect();
  if decoded.iter().all(fits) {
    return Ok(decoded);
  }
  let glyphs = Glyphs::load(api, true).await?;
  Ok(cards.iter().map(|c| glyphs.decode_value(c)).collect())
}

/// A job's page as the site's API gives it.
pub async fn info(api: &Api, id: &str) -> Result<Value> {
  let v = api
    .get(
      Route::Interns,
      INFO,
      &[("uuid", id.to_owned())],
      &refs::job_url(id),
    )
    .await?;
  let msg = v.at("msg");
  if msg.str("iname").is_none() {
    return Err(Error::not_found(format!(
      "Shixiseng shows no job at {} (it may have been taken down)",
      refs::job_url(id)
    )));
  }
  Ok(msg.clone())
}

/// A job's page: the whole description, pay, city and address, days a
/// week and months, degree, deadline, perks and skills, the company.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::job(arg)?;
  let info = info(api, &id).await?;
  parse::detail(&info, &id)
    .ok_or_else(|| Error::not_found(format!("Shixiseng shows no job at {}", refs::job_url(&id))))
}

/// A company's jobs, internships and campus jobs together, as its page
/// lists them.
pub async fn of_company(api: &Api, arg: &str, req: &PageReq) -> Result<Page<Post>> {
  let company = users::company(api, arg).await?;
  let page = req.number_or(1);
  let params = [
    ("uuid", company.id.clone()),
    ("type", "全部".to_owned()),
    ("page", page.to_string()),
    ("pagesize", COMPANY_PAGE.to_string()),
  ];
  let referer = refs::company_url(&company.id);
  let v = api
    .get(Route::Interns, COMPANY_JOBS, &params, &referer)
    .await?;
  let msg = v.at("msg");
  let rows = msg.list("data");
  let mut author = company.clone();
  author.raw = None;
  author.bio = None;
  let items: Vec<Post> = rows
    .iter()
    .filter_map(|row| parse::company_job(row, &author))
    .collect();
  let total = msg.u64("total").unwrap_or(0);
  let more = !rows.is_empty() && page * COMPANY_PAGE < total;
  Ok(Page::new(items, more.then(|| (page + 1).to_string())))
}
