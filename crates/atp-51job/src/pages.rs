//! Job and company pages of jobs.51job.com, read in the browser tab: the
//! page opens (the firewall's check passes by itself or is reported) and a
//! script reads its parts by the pages' markup into JSON: on a job page the
//! title, pay, the line of place, experience and degree (`.cn`), the
//! description (`.job_msg`), benefits (`.t1`), the rows under it (职能类别,
//! 上班地址) and the company (`.cname`, `.com_tag`); on a company page the
//! name, the line of ownership, size and industry (`.c-info`), the
//! introduction and the job cards (`a[sensorsdata]`, whose attribute holds
//! the job as JSON).

use atp_core::{Error, Result, Value, ValueExt};

use crate::api::Api;

/// The parts of a job page.
const JOB: &str = r#"(() => {
  const one = (s) => { const e = document.querySelector(s); return e ? e.innerText.trim() : ''; };
  const all = (s) => [...document.querySelectorAll(s)].map((e) => e.innerText.trim()).filter(Boolean);
  const head = (document.body ? document.body.innerText : '').slice(0, 3000);
  const box = document.querySelector('.bmsg.job_msg') || document.querySelector('.job_msg');
  let text = '';
  if (box) {
    const copy = box.cloneNode(true);
    copy.querySelectorAll('.fp, .mt10, script, style').forEach((n) => n.remove());
    text = copy.innerText.trim();
  }
  const company = document.querySelector('.cname a, .tCompany_sidebar .com_msg a');
  return {
    url: location.href,
    gone: /职位已下线|职位已删除|已经暂停招聘|页面不存在/.test(head),
    title: one('.cn h1') || one('h1'),
    salary: one('.cn strong'),
    meta: one('.cn .msg.ltype') || one('.msg.ltype'),
    text,
    welfare: all('.t1 span'),
    rows: all('.bmsg .fp'),
    company: company ? company.innerText.trim() : '',
    company_url: company ? company.href : '',
    company_tags: one('.com_tag'),
  };
})()"#;

/// The parts of a company page.
const COMPANY: &str = r#"(() => {
  const one = (s) => { const e = document.querySelector(s); return e ? e.innerText.trim() : ''; };
  const head = (document.body ? document.body.innerText : '').slice(0, 3000);
  const info = document.querySelector('.c-info.ellipsis') || document.querySelector('.c-info');
  const logo = document.querySelector('.c-logo img, .com_logo img');
  return {
    url: location.href,
    gone: /公司不存在|页面不存在/.test(head),
    name: one('h1') || one('.cname'),
    info: info ? [...info.querySelectorAll('span')].map((s) => s.innerText.trim()).filter(Boolean) : [],
    intro: one('#companyIntroRef') || one('.c-intro'),
    logo: logo ? logo.src : '',
    cards: [...document.querySelectorAll('a[sensorsdata]')].map((a) => ({
      href: a.href,
      data: a.getAttribute('sensorsdata') || '',
    })),
  };
})()"#;

/// Open `url` and read it with `script`; a page the site says is gone is
/// `not_found`.
async fn read(api: &Api, url: &str, script: &str, what: &str) -> Result<Value> {
  let tab = api.ctx.tab().await?;
  tab.goto(url).await?;
  let page = tab.eval(script).await?;
  if page.bool("gone") == Some(true) {
    return Err(Error::not_found(format!(
      "51job has no {what} at {url} (it may have been taken down)"
    )));
  }
  Ok(page)
}

pub async fn job(api: &Api, url: &str) -> Result<Value> {
  read(api, url, JOB, "job").await
}

pub async fn company(api: &Api, url: &str) -> Result<Value> {
  read(api, url, COMPANY, "company").await
}
