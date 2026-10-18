Companies' own career sites (校招官网), through the requests their pages
make: the big companies by a short name — ByteDance, Tencent, Alibaba,
Meituan, Kuaishou — and any site run on Feishu Hire (飞书招聘), Beisen
(北森) or Moka, by its link. That covers the official application links
Nowcoder's 校招 schedule shows (`apply_url` of `atp nowcoder schedule`),
most of which are Moka, Beisen or Feishu Hire sites. ATP searches a site's
jobs with the filters it has (kind of hiring, city, batch, category) and
reads a job's page; applying happens on the site.

## Logging in

There is nothing to log in to: every command works without an account.
Each company has an account system of its own, and ATP neither applies,
saves jobs nor reads applications on any of them; a job's page (`read`)
gives the link to apply at (`apply_url`). `atp careers status` therefore
always reports "not logged in". No environment variables are read.

The cookies sites hand a visitor (CSRF tokens, visitor sessions) live for
one command and are never saved. What ATP learns about a site from its
page (its website path, portal id, the company, Moka's IV) and the codes
of its filters are kept a day in the cache.

Risk control: ATP keeps at least 1.5 s between requests; the sites do not
publish their limits. A site that answers with a bot check or firewall
page instead of data (Alibaba's baxia on talent.alibaba.com, Aliyun's WAF,
captchas) is reported as `verification_required` with the page; ATP never
answers such checks and does not open them in a browser. ByteDance's and
Feishu Hire's pages sign their search and job requests (acrawler
`_signature`), which the sites do not require today; should one refuse an
unsigned request, ATP reports it and stops, as it does not sign.

## References

- Sites (`--site`, a global option of every command): a short name —
  `bytedance`, `tencent`, `alibaba`, `meituan`, `kuaishou` (or 字节跳动,
  腾讯, 阿里巴巴, 美团, 快手) — or a site's link, `https://` optional:
  - Feishu Hire: `https://jobs.bytedance.com/campus` (or `/experienced`),
    `https://<tenant>.jobs.feishu.cn/<website>` (or the host alone);
  - Beisen: `https://<tenant>.zhiye.com`, any of its pages
    (`/campus/jobs` and `/intern/…`, `/social/…` also set the kind);
  - Moka: `https://app.mokahr.com/campus-recruitment/<org>/<siteId>`, or
    `social-recruitment`, the mobile `/m/…` links and short links
    `https://app.mokahr.com/su/<code>`;
  - `join.qq.com`, `careers.tencent.com`, `campus-talent.alibaba.com`,
    `talent.alibaba.com`, `zhaopin.meituan.com`, `campus.kuaishou.cn`;
  - any other link: ATP reads the page and recognizes a Moka, Feishu Hire
    or Beisen site on a company's own domain (滴滴's
    `https://campus.didiglobal.com/campus_apply/didiglobal/96064` is Moka,
    京东方's `https://campus.boe.com` is Beisen). What it found is kept a
    day.
  Queries and `#/…` parts of links (`?locale=zh-CN#/jobs`, share tokens)
  are ignored.
- Jobs: their link, which names the site, so `read` needs no `--site`:
  `https://jobs.bytedance.com/campus/position/<id>/detail`,
  `https://<tenant>.jobs.feishu.cn/<website>/position/<id>/detail`,
  `https://join.qq.com/post_detail.html?postid=<id>`,
  `https://careers.tencent.com/jobdesc.html?postId=<id>`,
  `https://campus-talent.alibaba.com/campus/position/<id>`,
  `https://talent.alibaba.com/off-campus/position-detail?positionId=<id>`,
  `https://zhaopin.meituan.com/web/position/detail?jobUnionId=<id>`,
  `https://campus.kuaishou.cn/recruit/campus/e/#/campus/job-info/<id>`,
  `https://<tenant>.zhiye.com/<campus|intern|social>/detail?jobAdId=<id>`,
  `https://app.mokahr.com/campus-recruitment/<org>/<siteId>#/job/<id>`.
  A job's id alone works with `--site`.
- `#N` works after any list.

## Commands

### search

`search WORDS --site SITE` searches the site's jobs by words, 20 a page
(or as many as asked, on sites paging by offset). `--filter campus`,
`intern` or `social` keeps one kind of hiring where the site tells them
apart (see the sites below); without it, a site lists what its campus
pages list, campus jobs and internships together. `jobs` narrows further.

### jobs

`jobs [WORDS] --site SITE` is the search with the filters the site has;
without words it lists every job they let through.

- `--kind campus|intern|social` (校招, 实习, 社招 too): campus jobs for new
  graduates, internships, experienced hires. On ByteDance, Tencent and
  Alibaba, `social` reads the company's experienced-hire site.
- `--city`: a city by name (北京, 深圳; 北京市 matches 北京) or by the
  site's code from `cities`.
- `--batch ID`: a batch or project (2027届校招, 日常实习 …) by its id from
  `batches`.
- `--category`: a category by name (技术类, 后端 for 研发/后端) or its id
  from `categories`.

A filter a site does not have is refused, not ignored.

    atp careers jobs --site bytedance 后端 --kind intern --city 深圳
    atp careers jobs --site 'https://dcits.zhiye.com/campus/jobs' -n 50

### read

A job's page: the title, the whole description and requirements, the
company (`company`, and as the `author` with its site), the cities, the
kind of hiring, category, department and batch where the site names them,
and `apply_url`, the page to apply on (see Job posts below).

### sites

The short names with their companies and sites, then the vendors whose
sites ATP reads by their link.

### batches, cities, categories

The codes of a site's filters, for `jobs --batch`, `--city` and
`--category`: batches or projects with what they are for, cities, and
categories (with the ones under them, `研发/后端`). `--kind` picks the
site whose codes to list where a company keeps kinds apart (ByteDance's
experienced-hire site has its own; on Alibaba, `--kind intern` lists the
cities and categories of the internship batch). Meituan, Beisen and Moka
sites list none.

    atp careers batches --site alibaba
    atp careers cities --site tencent

## Sites

What each site's search narrows by, beside words:

| Site | Kinds | City | Batch | Category |
| --- | --- | --- | --- | --- |
| ByteDance (jobs.bytedance.com) | campus, intern; social: `/experienced` | yes | subjects | yes |
| Feishu Hire sites | campus, intern, social | yes | subjects | yes |
| Tencent (join.qq.com) | campus, intern; social: careers.tencent.com | yes | projects | yes |
| Alibaba (campus-talent.alibaba.com) | by batch; social: talent.alibaba.com | yes | batches | yes |
| Meituan (zhaopin.meituan.com) | campus, intern, social | yes | — | — |
| Kuaishou (campus.kuaishou.cn) | campus, intern | yes | projects | yes |
| Beisen sites | campus, intern, social | — | — | — |
| Moka sites | campus, intern (on campus sites) | — | — | — |

- ByteDance and Feishu Hire: a company's Feishu Hire domain may hold
  several websites (`/campus`, `/graduate`, `/social` …); the link's
  first path names one, the host alone the default one.
- Tencent: without `--kind` or `--batch`, join.qq.com searches every open
  project (校园招聘, 实习, 青云计划). careers.tencent.com (social) takes
  words only.
- Alibaba: campus searches cover one batch: `--batch`, else the first
  graduate batch (`--kind intern`: the first internship batch). Its
  special talent plans (阿里星) are not listed. talent.alibaba.com
  (social) takes words only, and its pages load Alibaba's baxia bot check.
- Meituan: without `--kind`, campus jobs and internships; cities are
  looked up by name.
- Kuaishou: without `--batch`, the newest open projects of campus jobs
  and internships. Its experienced hires are on zhaopin.kuaishou.cn, not
  read.
- Beisen: the kind comes from `--kind` or the link (`/campus/…`); a job
  link's path also tells the kind its page is read with.
- Moka: a site lists campus jobs and internships or experienced hires;
  `--kind` picks 全职 or 实习 on campus sites. Answers come encrypted
  (AES-CBC with the key each answer names and the IV of the site's
  page); ATP decrypts them as the page does.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in
`extra`: `company`, `city` (cities joined by `/`), `job_type` (`full-time`
or `intern`), `experience` and `degree` (as the site words them, where it
does), `deadline` (Beisen), and `salary` where a site shows pay (Beisen,
mostly 面议; campus sites rarely do).

The careers keys: `recruit_type` (`campus`, `intern`, `social`),
`category`, `department`, `batch` (the project or batch), `job_code`,
`headcount`, `address`; Tencent's `interview_cities`, Alibaba's
`graduation` (who the batch is for) and `interview_cities`, Kuaishou's
`applicants`; and on job pages `apply_url`. `created_at` is when the site
published the job, where it tells; Tencent's campus jobs tell no time.

Recruiters and other staff (their names, accounts, contact details and
QR codes) are dropped from every payload, `raw` included.

## Not supported yet

- Pinduoduo (careers.pddglobalhr.com): every request carries its
  anti_content risk token.
- Xiaohongshu (job.xiaohongshu.com, its own ats-website): its requests
  are signed as on xiaohongshu.com; possible later through the signer of
  `atp xhs`.
- Baidu (talent.baidu.com), JD (campus.jd.com), NetEase (campus.163.com),
  Huawei (career.huawei.com): sites of their own, behind captchas.
- Kuaishou's experienced hires (zhaopin.kuaishou.cn), and other vendors'
  sites (hotjob.cn, 51job's campus pages, iguopin.com, zhaopin.com's
  company pages).

## Limits

- No applying, saving jobs or reading applications: each company's
  account system is its own (see Logging in).
- Beisen and Moka sites are searched by words and kind only; their
  cities, departments and project filters are not offered.
- A site's own search decides what words match (titles, descriptions);
  sites differ.
- Job pages are read one at a time; lists carry what each site's list
  carries (join.qq.com's list has no description, careers.tencent.com's
  the duties only).
