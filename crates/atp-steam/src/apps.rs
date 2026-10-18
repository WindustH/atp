//! The store: search, app pages with their review summary, the featured
//! lists of the front page, and apps' prices in several storefronts.

use std::collections::HashSet;

use atp_core::paging::slice;
use atp_core::{Error, Page, PageReq, Post, Query, Result, Value, ValueExt};

use crate::api::{self, Api};
use crate::{parse, refs, reviews};

/// `hot --category`: the lists of `featuredcategories`.
pub const HOT_CATEGORIES: &[&str] = &["top-sellers", "specials", "new-releases", "coming-soon"];

/// Store matches for a name; the store answers one short list.
pub async fn search(api: &Api, query: &Query, page: &PageReq) -> Result<Page<Post>> {
  let params = vec![
    ("term", query.keyword.clone()),
    ("cc", api.cc()?),
    ("l", api.language()),
  ];
  let v = api.store("api/storesearch/", params).await?;
  let items = v.list("items").iter().map(parse::search_item).collect();
  Ok(slice(items, page))
}

/// An app page, with the summary of its reviews in every language.
pub async fn read(api: &Api, arg: &str) -> Result<Post> {
  let id = refs::app(arg)?;
  let data = details(api, id, &api.cc()?).await?;
  let summary = reviews::summary(api, id).await.ok();
  Ok(parse::app(&data, summary.as_ref()))
}

/// `appdetails` of one app in the store of `cc`.
async fn details(api: &Api, id: u64, cc: &str) -> Result<Value> {
  let params = vec![
    ("appids", id.to_string()),
    ("cc", cc.to_owned()),
    ("l", api.language()),
  ];
  let v = api.store("api/appdetails", params).await?;
  let entry = v.at(&id.to_string());
  if entry.bool("success") != Some(true) {
    return Err(
      Error::not_found(format!("the Steam store ({cc}) has no app {id}"))
        .with_hint("it may not exist, be removed, or not be sold in that country (STEAM_CC)"),
    );
  }
  Ok(entry.at("data").clone())
}

/// One of the front page's lists, each app once.
pub async fn hot(api: &Api, category: Option<&str>, page: &PageReq) -> Result<Page<Post>> {
  let key = category.unwrap_or(HOT_CATEGORIES[0]).replace('-', "_");
  let params = vec![("cc", api.cc()?), ("l", api.language())];
  let v = api.store("api/featuredcategories/", params).await?;
  let mut seen = HashSet::new();
  let items = v
    .list(&format!("{key}.items"))
    .iter()
    .map(parse::featured)
    .filter(|post| seen.insert(post.id.clone()))
    .collect();
  Ok(slice(items, page))
}

/// `prices APP… [--cc CC…]`: each app in each storefront, the app's page
/// read in the first one and only prices (several apps at once) in the
/// others.
pub async fn prices(api: &Api, apps: &[String], countries: &[String]) -> Result<Page<Post>> {
  let countries = match countries {
    [] => vec![api.cc()?],
    given => given
      .iter()
      .map(|cc| api::country(cc))
      .collect::<Result<Vec<_>>>()?,
  };
  let ids = apps
    .iter()
    .map(|app| refs::app(&api.ctx.post_ref(app)?))
    .collect::<Result<Vec<_>>>()?;
  // Each app's name and whether it is free, from its page.
  let mut known = Vec::with_capacity(ids.len());
  let mut posts = Vec::new();
  for id in &ids {
    let data = details(api, *id, &countries[0]).await?;
    let app = (data.str("name"), data.bool("is_free") == Some(true));
    posts.push(priced(*id, &app, &data, &countries[0]));
    known.push(app);
  }
  for cc in &countries[1..] {
    let joined: Vec<String> = ids.iter().map(u64::to_string).collect();
    let params = vec![
      ("appids", joined.join(",")),
      ("filters", "price_overview".to_owned()),
      ("cc", cc.clone()),
    ];
    let v = api.store("api/appdetails", params).await?;
    for (id, app) in ids.iter().zip(&known) {
      let data = v.at(&format!("{id}.data"));
      posts.push(priced(*id, app, data, cc));
    }
  }
  Ok(Page::last(posts))
}

/// An app's price in one storefront (`data` holds `price_overview`; free
/// and unsold apps have none); `app` is its name and whether it is free.
fn priced(id: u64, app: &(Option<String>, bool), data: &Value, cc: &str) -> Post {
  let page = parse::app(data, None);
  let mut post = Post {
    kind: "price".into(),
    title: app.0.clone(),
    url: Some(format!("{}?cc={cc}", parse::app_url(&id.to_string()))),
    price: page.price,
    id: id.to_string(),
    ..Post::default()
  };
  post
    .extra
    .insert("cc".into(), cc.to_ascii_uppercase().into());
  let state = if post.price.is_some() {
    None
  } else if app.1 {
    Some("free")
  } else {
    Some("no price in this store")
  };
  if let Some(state) = state {
    post.text = Some(state.into());
  }
  post
}
