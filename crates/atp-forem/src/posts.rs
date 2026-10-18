//! Writing with the API key: a new article, published or kept as a draft
//! (`POST /api/articles`), and changes to one of yours (`PUT
//! /api/articles/<id>`): its title, Markdown, tags, and whether it is
//! published. Forem's API has no way to delete an article or to upload
//! images, and none to comment.

use atp_core::cli::{Change, confirm};
use atp_core::http::Method;
use atp_core::output::note;
use atp_core::{Action, Draft, Edit, Error, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::refs::{self, ArticleRef};

/// Tags an article may have.
const TAGS_MAX: usize = 4;
/// Rows of `/api/articles/me/all` asked for at once, and pages looked at.
const OWN_PAGE: usize = 1000;
const OWN_PAGES: u32 = 10;

/// `post`: an article from the text or `--article FILE`, published, or a
/// draft with `--visibility private`.
pub async fn publish(api: &Api, draft: &Draft) -> Result<Action> {
  api.require()?;
  refuse(draft)?;
  let body = body(draft)?;
  if draft.title.is_none() && !titled(&body) {
    return Err(Error::input(
      "an article needs --title (or a `title:` in its front matter)",
    ));
  }
  let tags = tags(&draft.topics)?;
  let published = published(draft.visibility.as_deref())?;
  let host = api.host();
  let mut lines = vec![format!("New article on {host}:")];
  lines.push(format!(
    "  title       {}",
    draft.title.as_deref().unwrap_or("(from the front matter)")
  ));
  lines.push(format!(
    "  tags        {}",
    if tags.is_empty() {
      "(none)".into()
    } else {
      tags.join(", ")
    }
  ));
  lines.push(format!("  Markdown    {} characters", body.chars().count()));
  lines.push(format!("  state       {}", state(published)));
  note(&lines.join("\n"));
  let what = if published {
    "publish this article"
  } else {
    "save this draft"
  };
  confirm(draft.yes, &format!("{what} on {host}"))?;
  let mut article = json!({ "body_markdown": body, "published": published });
  if let Some(title) = &draft.title {
    article["title"] = json!(title);
  }
  if !tags.is_empty() {
    article["tags"] = json!(tags);
  }
  let v = api
    .write(Method::POST, "articles", json!({ "article": article }))
    .await?;
  Ok(done(if published { "publish" } else { "draft" }, &v))
}

/// Options Forem's API cannot honour.
fn refuse(draft: &Draft) -> Result<()> {
  if !draft.images.is_empty() {
    return Err(Error::unsupported(
      "post --image (Forem's API takes no uploads; link images in the Markdown)",
    ));
  }
  if draft.cover.is_some() {
    return Err(Error::unsupported(
      "post --cover (Forem takes a cover only as a link: `cover_image: URL` in the front matter)",
    ));
  }
  if draft.reply_to.is_some() || draft.quote.is_some() {
    return Err(Error::unsupported(
      "post --reply-to / --quote (Forem's API has no comments or reposts)",
    ));
  }
  Ok(())
}

/// The Markdown: the text, or the file of `--article`.
fn body(draft: &Draft) -> Result<String> {
  let Some(path) = &draft.article else {
    return Ok(draft.text.clone());
  };
  if !draft.text.is_empty() {
    return Err(Error::input(
      "give the article as TEXT or --article FILE, not both",
    ));
  }
  std::fs::read_to_string(path)
    .map_err(|e| Error::input(format!("cannot read {}: {e}", path.display())))
}

/// Whether Markdown opens with front matter that names a title.
fn titled(body: &str) -> bool {
  let Some(rest) = body.trim_start().strip_prefix("---") else {
    return false;
  };
  rest
    .lines()
    .take_while(|line| line.trim() != "---")
    .any(|line| line.trim_start().starts_with("title:"))
}

/// `--topic` values as Forem's tags (`#Rust` → `rust`).
fn tags(topics: &[String]) -> Result<Vec<String>> {
  let tags: Vec<String> = topics
    .iter()
    .map(|t| t.trim().trim_start_matches('#').to_lowercase())
    .filter(|t| !t.is_empty())
    .collect();
  if tags.len() > TAGS_MAX {
    return Err(Error::input(format!("Forem takes at most {TAGS_MAX} tags")));
  }
  Ok(tags)
}

/// `--visibility`: `public` publishes, `private` keeps a draft.
fn published(visibility: Option<&str>) -> Result<bool> {
  match visibility {
    None | Some("public") => Ok(true),
    Some("private") => Ok(false),
    Some(other) => Err(Error::input(format!(
      "Forem has no {other} articles: `public` publishes, `private` keeps a draft"
    ))),
  }
}

fn state(published: bool) -> &'static str {
  if published { "published" } else { "draft" }
}

fn done(action: &str, v: &Value) -> Action {
  let id = v.str("id").unwrap_or_default();
  let mut done = Action::done(action, &id).with_id(&id);
  if let Some(url) = v.str("url") {
    done = done.with_url(url);
  }
  done
}

/// `edit`: change one of your articles (published or a draft).
pub async fn edit(api: &Api, arg: &str, edit: &Edit) -> Result<Action> {
  api.require()?;
  let current = own(api, &refs::article(arg)?).await?;
  let id = current.u64("id").unwrap_or_default();
  let host = api.host();
  let mut change = Change::new(format!("Article {id} on {host}:"));
  let mut article = serde_json::Map::new();
  if let Some(title) = &edit.title {
    change.field("title", &current.str("title").unwrap_or_default(), title);
    article.insert("title".into(), json!(title));
  }
  if let Some(text) = &edit.text {
    change.field(
      "Markdown",
      &current.str("body_markdown").unwrap_or_default(),
      text,
    );
    article.insert("body_markdown".into(), json!(text));
  }
  if let Some(topics) = &edit.topics {
    let new = tags(topics)?;
    let old: Vec<String> = current
      .list("tag_list")
      .iter()
      .filter_map(|t| t.as_str().map(str::to_owned))
      .collect();
    change.field("tags", &old.join(", "), &new.join(", "));
    article.insert("tags".into(), json!(new));
  }
  if let Some(visibility) = &edit.visibility {
    let published = published(Some(visibility))?;
    let was = current.bool("published").unwrap_or(true);
    change.field("state", state(was), state(published));
    article.insert("published".into(), json!(published));
  }
  change.confirm(edit.yes, &format!("change this article on {host}"))?;
  let path = format!("articles/{id}");
  let v = api
    .write(Method::PUT, &path, json!({ "article": article }))
    .await?;
  Ok(done("edit", &v))
}

/// One of your articles, published or not, as `/api/articles/me/all`
/// lists it (with its Markdown).
async fn own(api: &Api, article: &ArticleRef) -> Result<Value> {
  for page in 1..=OWN_PAGES {
    let query = vec![
      ("page", page.to_string()),
      ("per_page", OWN_PAGE.to_string()),
    ];
    let v = api.get("articles/me/all", query).await?;
    let rows = v.list("");
    let found = rows.iter().find(|row| match article {
      ArticleRef::Id(id) => row.u64("id") == Some(*id),
      ArticleRef::Path { slug, .. } => row.str("slug").as_deref() == Some(slug.as_str()),
    });
    if let Some(found) = found {
      return Ok(found.clone());
    }
    if rows.len() < OWN_PAGE {
      break;
    }
  }
  Err(Error::not_found(format!(
    "no such article among yours on {}",
    api.host()
  )))
}
