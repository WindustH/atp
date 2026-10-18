//! Job and company pages of www.zhipin.com, read in the browser tab: the
//! page opens (passing the site's security page first when it shows
//! one, by the site's own scripts) and a script reads its parts by the
//! pages' markup into JSON, once the document is parsed.

use atp_core::{Error, Result, Value, ValueExt};

use crate::api::Api;

/// What a job page shows: title, pay, the line of city, experience and
/// degree, the description, skills and benefits, the address, the
/// company with its facts, and the link of the 立即沟通 button (which
/// carries the job's `securityId` and `lid`).
const JOB: &str = r#"() => {
  const one = (s) => { const e = document.querySelector(s); return e ? (e.innerText || e.textContent || '').trim() : ''; };
  const first = (list) => { for (const s of list) { const t = one(s); if (t) return t; } return ''; };
  const all = (s) => [...document.querySelectorAll(s)].map((e) => (e.innerText || e.textContent || '').trim()).filter(Boolean);
  const head = (document.body ? document.body.innerText : '').slice(0, 4000);
  const link = document.querySelector('.job-sider a[href*="/gongsi/"], .sider-company a[href*="/gongsi/"], .company-info a[href*="/gongsi/"], a[ka*="company"][href*="/gongsi/"]');
  const logo = document.querySelector('.sider-company img, .company-info img');
  const start = [...document.querySelectorAll('[data-url], [redirect-url]')]
    .map((e) => e.getAttribute('data-url') || e.getAttribute('redirect-url') || '')
    .find((u) => u.includes('securityId='));
  return {
    url: location.href,
    gone: /职位已关闭|职位已下线|职位已失效|该职位已|页面不存在|职位不存在/.test(head),
    name: first(['.job-banner .name h1', '.job-primary .name h1', '.info-primary .name h1', '.name h1', 'h1']),
    salary: first(['.job-banner .salary', '.job-primary .salary', '.info-primary .salary', '.name .salary']),
    city: first(['.job-banner .text-city', '.job-primary .text-city', '.text-city']),
    experience: first(['.text-experiece', '.text-experience']),
    degree: one('.text-degree'),
    description: first(['.job-detail .job-sec-text:not(.fold-text)', '.job-sec-text:not(.fold-text)', '.job-sec-text']),
    skills: all('.job-keyword-list li'),
    welfare: all('.job-banner .job-tags span, .job-primary .job-tags span'),
    address: first(['.location-address', '.job-location .location-address']),
    company: link ? (link.getAttribute('title') || link.innerText || '').trim() : '',
    company_url: link ? link.href : '',
    company_logo: logo ? logo.src : '',
    company_facts: all('.sider-company > p').filter((t) => t !== '公司基本信息'),
    start: start || '',
  };
}"#;

/// What a company page shows: name, logo, facts, introduction, address,
/// and the jobs it links.
const COMPANY: &str = r#"() => {
  const one = (s) => { const e = document.querySelector(s); return e ? (e.innerText || e.textContent || '').trim() : ''; };
  const first = (list) => { for (const s of list) { const t = one(s); if (t) return t; } return ''; };
  const all = (s, root) => [...(root || document).querySelectorAll(s)].map((e) => (e.innerText || e.textContent || '').trim()).filter(Boolean);
  const head = (document.body ? document.body.innerText : '').slice(0, 4000);
  const logo = document.querySelector('.company-banner img, .info-primary img, .company-logo img');
  const jobs = [...document.querySelectorAll('a[href*="/job_detail/"]')].map((a) => ({
    href: a.href,
    name: (a.querySelector('.job-name, .name') || a).innerText.trim().split('\n')[0],
    salary: (a.querySelector('.salary, .red') || {}).innerText || '',
    meta: all('.job-text span, .job-limit span, p span', a),
  }));
  return {
    url: location.href,
    gone: /公司不存在|页面不存在/.test(head),
    name: first(['.company-banner .info h1', '.info-primary h1', '.company-banner h1', 'h1']),
    logo: logo ? logo.src : '',
    facts: all('.company-banner .info p span, .info-primary p span'),
    intro: first(['.job-sec .fold-text', '.company-intro .text', '.job-sec .text']),
    address: first(['.location-address', '.company-address']),
    jobs,
  };
}"#;

/// `read` run on the document once it is parsed.
fn when_parsed(read: &str) -> String {
  format!(
    "new Promise((done) => {{ const read = {read}; const go = () => done(read()); \
     if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', go, \
     {{once: true}}); else go(); }})"
  )
}

/// Open `url` and read it with `script`; a page the site says is gone is
/// `not_found`.
async fn read(api: &Api, url: &str, script: &str, what: &str) -> Result<Value> {
  let page = api.page(url, &when_parsed(script)).await?;
  if page.bool("gone") == Some(true) {
    return Err(Error::not_found(format!(
      "BOSS Zhipin has no {what} at {url} (it may have been taken down)"
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
