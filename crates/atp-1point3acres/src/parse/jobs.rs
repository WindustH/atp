//! Hiring posts as jobs: threads of the 内推 boards that offer a job
//! (海外职位内推 and 中国招聘信息 with the types 我司要招人 or Hiring Manager
//! 直招, and every thread of ✨企业直聘, where employers post). Requests
//! for a referral (求内推) and every other thread stay threads.
//!
//! The job keys come from what the thread shows: `company` from its
//! company tag, `city` from its place tag, and the pay where its title
//! names it ([`Salary::in_text`]).

use atp_core::jobs::Salary;
use atp_core::{Post, Value, ValueExt, json};

use super::plain;

/// Boards and the thread types in them that offer a job.
const HIRING: &[(u64, &[u64])] = &[
  // 海外职位内推: 我司要招人, Hiring Manager 直招.
  (198, &[653, 975]),
  // 中国招聘信息: 我司要招人, Hiring Manager 直招.
  (322, &[1009, 1011]),
];
/// ✨企业直聘: employers' own posts.
const EMPLOYERS: u64 = 266;
/// Kinds of topic tags.
const COMPANY_TAG: u64 = 3;
const PLACE_TAG: u64 = 4;

pub fn is_job(v: &Value) -> bool {
  let Some(fid) = v.u64("fid") else {
    return false;
  };
  let typeid = v.u64("typeid").unwrap_or_default();
  fid == EMPLOYERS
    || HIRING
      .iter()
      .any(|(board, types)| *board == fid && types.contains(&typeid))
}

/// The name of the first topic tag of a kind.
fn tag(v: &Value, kind: u64) -> Option<String> {
  v.list("topic_tag")
    .iter()
    .find(|t| t.u64("category") == Some(kind))
    .and_then(|t| t.str("tagname"))
    .map(|name| plain(&name))
}

pub fn fill(post: &mut Post, v: &Value) {
  post.kind = "job".into();
  if let Some(company) = tag(v, COMPANY_TAG) {
    post.extra.insert("company".into(), json!(company));
  }
  if let Some(city) = tag(v, PLACE_TAG) {
    post.extra.insert("city".into(), json!(city));
  }
  if let Some(salary) = post.title.as_deref().and_then(Salary::in_text) {
    salary.fill(&mut post.extra);
  }
}
