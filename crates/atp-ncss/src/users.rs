//! Companies, which are NCSS's users here (`/student/jobs/<id>/corp.html`),
//! and the logged-in student, as far as the pages tell: whether a student
//! is logged in, not who.

use atp_core::{Error, Result, User};

use crate::api::Api;
use crate::{parse, refs};

/// The logged-in student. The pages ATP reads say that a student is logged
/// in, not the account's name.
pub async fn whoami(api: &Api) -> Result<User> {
  api.own_page("/student/jobs/index.html").await?;
  Ok(User {
    id: "student".into(),
    name: "NCSS student (学信网 account)".into(),
    ..User::default()
  })
}

/// A company's page: name, logo, introduction, ownership, industry,
/// fields, size, website and address.
pub async fn company(api: &Api, arg: &str) -> Result<User> {
  let id = refs::company(arg)?;
  let html = api.page(&format!("/student/jobs/{id}/corp.html")).await?;
  parse::company(&html, &id).ok_or_else(|| {
    Error::not_found(format!(
      "NCSS shows no company at {}",
      refs::company_url(&id)
    ))
  })
}
