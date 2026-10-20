//! Search, as the app's search box asks: outside a group
//! (`/v2/search/topics`, `scope=all`) or inside one
//! (`/v2/search/groups/<gid>/topics`), in the app's default order, paged
//! by the `index` each page returns.

use atp_core::{Error, Page, PageReq, Post, Result, ValueExt};

use crate::api::Api;
use crate::parse;

/// The page size of the app.
const PAGE: usize = 20;

/// Topics of every group the site searches for you.
pub async fn all(api: &Api, keyword: &str, req: &PageReq) -> Result<Page<Post>> {
  let tail = [("scope", "all"), ("order_by", "default")];
  topics(api, "/v2/search/topics", keyword, &tail, req).await
}

/// Topics of one group.
pub async fn group(api: &Api, gid: &str, keyword: &str, req: &PageReq) -> Result<Page<Post>> {
  let path = format!("/v2/search/groups/{gid}/topics");
  topics(api, &path, keyword, &[("order_by", "default")], req).await
}

async fn topics(
  api: &Api,
  path: &str,
  keyword: &str,
  tail: &[(&'static str, &str)],
  req: &PageReq,
) -> Result<Page<Post>> {
  let keyword = keyword.trim();
  if keyword.is_empty() {
    return Err(Error::input("search for something: the words are empty"));
  }
  api.need_login()?;
  let count = req.size_within(PAGE);
  let index = req.number_or(0);
  let mut params = vec![
    ("keyword", keyword.to_owned()),
    ("count", count.to_string()),
  ];
  if index > 0 {
    params.push(("index", index.to_string()));
  }
  params.extend(tail.iter().map(|(k, v)| (*k, (*v).to_owned())));
  let v = api.get(path, &params).await?;
  let found = v.list("topics");
  let posts = found.iter().filter_map(parse::topic).collect();
  let next = (found.len() >= count).then(|| {
    v.u64("index")
      .filter(|n| *n > index)
      .unwrap_or(index + found.len() as u64)
      .to_string()
  });
  Ok(Page::new(posts, next))
}
