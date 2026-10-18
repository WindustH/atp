//! The job of a job post, in the shared keys of `Post.extra`.
//!
//! Eleduck's job posts carry no structured fields beyond their tags, one
//! per tag group: 工作方式 (`job_type` group: 全职远程, 全职坐班, 线上兼职,
//! 线下兼职, 混合办公), 招聘类型 (`hire_type`: 企业直招, 外包零活 …),
//! 职业 (`skill_type`: 开发, 产品 …), 城市 (`city`) and 领域 (`field`, several).
//! From them come `remote`, `job_type`, `city` and the platform's own
//! `hire`, `role` and `fields`. Posts without a 工作方式 tag (the curated
//! 精选职位推荐, `【免费推荐/全职远程】…`) are read from the words of their
//! title. The pay comes from the title ([`super::salary`]).

use atp_core::{Post, Value, ValueExt, json};

use super::salary;

/// The tag of a group (by its code), as the site names it.
fn tag(v: &Value, group: &str) -> Option<String> {
  tags(v, group).into_iter().next()
}

fn tags(v: &Value, group: &str) -> Vec<String> {
  v.list("tags")
    .iter()
    .filter(|t| t.str("tag_group.code").as_deref() == Some(group))
    .filter_map(|t| t.str("name"))
    .collect()
}

/// `remote` and `job_type` of a 工作方式 tag; 混合办公 (hybrid) says
/// neither for sure.
fn work_mode(mode: &str) -> (Option<bool>, Option<&'static str>) {
  match mode {
    "全职远程" => (Some(true), Some("full-time")),
    "全职坐班" => (Some(false), Some("full-time")),
    "线上兼职" => (Some(true), Some("part-time")),
    "线下兼职" => (Some(false), Some("part-time")),
    _ => (None, None),
  }
}

/// `remote` and `job_type` from the words of a title.
fn from_title(title: &str) -> (Option<bool>, Option<&'static str>) {
  let remote = (title.contains("远程") || title.to_lowercase().contains("remote")).then_some(true);
  let job_type = if title.contains("实习") {
    Some("intern")
  } else if title.contains("兼职") {
    Some("part-time")
  } else if title.contains("全职") {
    Some("full-time")
  } else {
    None
  };
  (remote, job_type)
}

pub fn fill(post: &mut Post, v: &Value) {
  let title = post.title.clone().unwrap_or_default();
  let (remote, mut job_type) = match tag(v, "job_type") {
    Some(mode) => work_mode(&mode),
    None => from_title(&title),
  };
  let hire = tag(v, "hire_type");
  // 外包零活: gigs and outsourced work.
  if hire.as_deref() == Some("外包零活") {
    job_type = Some("freelance");
  }
  let extra = &mut post.extra;
  if let Some(remote) = remote {
    extra.insert("remote".into(), json!(remote));
  }
  if let Some(job_type) = job_type {
    extra.insert("job_type".into(), json!(job_type));
  }
  if let Some(city) = tag(v, "city").filter(|c| c != "其他") {
    extra.insert("city".into(), json!(city));
  }
  if let Some(hire) = hire {
    extra.insert("hire".into(), json!(hire));
  }
  if let Some(role) = tag(v, "skill_type") {
    extra.insert("role".into(), json!(role));
  }
  let fields = tags(v, "field");
  if !fields.is_empty() {
    extra.insert("fields".into(), json!(fields));
  }
  if let Some(pay) = salary::parse(&title) {
    pay.fill(extra);
  }
}
