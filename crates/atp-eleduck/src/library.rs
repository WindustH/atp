//! Your own lists: saved posts (收藏, `/profile/marks`), the posts you
//! viewed in the last three months (`/profile/views`) and your job
//! applications (`/job_applications/my_applications`).

use atp_core::{Page, PageReq, Post, Result, Value, ValueExt, json};

use crate::api::Api;
use crate::parse;

/// `applications --status`, as the site's tabs group the states.
fn application_status(status: &str) -> &'static str {
  match status {
    "processing" => "pending,contact_pending",
    "rejected" => "rejected",
    "withdrawn" => "withdrawn",
    "qualified" => "qualified",
    "expired" => "expired",
    _ => "",
  }
}

/// The posts inside the rows of `key`, each row's time in `extra.<at>`.
fn rows(v: &Value, key: &str, time: &str, at: &str) -> Page<Post> {
  let items = v
    .list(key)
    .iter()
    .filter(|row| !row.at("post").is_null())
    .map(|row| {
      let mut post = parse::post(row.at("post"));
      if let Some(when) = row.str(time) {
        post.extra.insert(at.into(), json!(when));
      }
      post
    })
    .collect();
  Page::new(items, parse::next_page(v))
}

pub async fn favorites(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let params = [("page", req.number_or(1).to_string())];
  let v = api.get_own("/profile/marks", &params).await?;
  Ok(rows(&v, "marks", "created_at", "saved_at"))
}

pub async fn history(api: &Api, req: &PageReq) -> Result<Page<Post>> {
  let params = [("page", req.number_or(1).to_string())];
  let v = api.get_own("/profile/views", &params).await?;
  Ok(rows(&v, "views", "updated_at", "viewed_at"))
}

/// Rows per page of applications, as the site asks for them.
const APPLICATIONS_PAGE: &str = "10";

pub async fn applications(api: &Api, status: Option<&str>, req: &PageReq) -> Result<Page<Post>> {
  let mut params = vec![
    ("page", req.number_or(1).to_string()),
    ("per_page", APPLICATIONS_PAGE.to_owned()),
  ];
  if let Some(status) = status.map(application_status).filter(|s| !s.is_empty()) {
    params.push(("status", status.to_owned()));
  }
  let v = api
    .get_own("/job_applications/my_applications", &params)
    .await?;
  let items = v
    .list("job_applications")
    .iter()
    .map(parse::application)
    .collect();
  Ok(Page::new(items, parse::next_page(&v)))
}
