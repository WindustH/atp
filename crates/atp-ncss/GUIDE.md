NCSS (国家大学生就业服务平台, www.ncss.cn), the national job board the
Ministry of Education runs for college graduates, through the pages of
its student side and the requests they make: the job search with its
filters (area, job type, pay, degree, category, industry, company
ownership, where the job was posted), job pages, company pages and their
jobs, and the job fairs (专场招聘) with their jobs and companies. Jobs
come from employers on the platform, from the provinces' and schools'
own job sites and from partner job sites; each says where it was posted.
With a login: saving a job and applying to one job at a time.

## Logging in

### Browser or cookie

The site's accounts are 学信网 (CHSI) accounts: its login page sends the
browser to account.chsi.com.cn, and the site then keeps the session in
its own cookies (`SESSION` and the others of www.ncss.cn). ATP takes that
session from a browser; it does not log in to 学信网 itself (password,
texted code and its captchas).

- `atp ncss login --browser` reads the cookies of a browser logged in at
  www.ncss.cn as a student.
- `atp ncss login --cookie '…'`: a `Cookie` header copied from the
  browser's developer tools (any request to `www.ncss.cn/student/…`).
- `ATP_NCSS_COOKIE='…'` passes the cookies for one run; they are never
  saved.

Every visit gets a `SESSION` cookie, logged in or not, so ATP checks the
login on the site: its pages say whether a student is logged in. They do
not show the account's name to ATP, so `whoami` names a student without
saying who. An employer's or a manager's account is refused.

### Without an account

`search`, `jobs`, `read`, `user`, `user-posts`, `fairs`, `fair`,
`fair-companies`, `areas`, `categories` and `industries` work without
logging in. The job search shows its first ten pages (200 jobs) only;
further pages want a login (`not_authenticated`).

### With an account

`whoami`, `favorite` (saving a job), `apply`, and the job search past its
tenth page. None of them has been tried with a real account: they follow
the site's current pages, and the shapes of their answers are read
defensively.

### How long it lasts

How long the site keeps a session is not published. When it has ended,
the pages no longer show a student and commands that need one fail with
`not_authenticated` and the hint to log in again.

### Risk control

- ATP keeps at least 2 s between requests and waits 2 to 5 s more before
  saving or applying. The site does not publish its limits.
- The site sits behind Aliyun's firewall. A check page in place of an
  answer is `verification_required`; on a terminal ATP opens it in your
  browser with the session's cookies and retries once. It never solves
  checks itself.
- Applying sends the résumé you keep on the site to the employer; one job
  per command, after showing it and asking.

### Environment

`ATP_NCSS_COOKIE`: the session's cookies for one run.

## References

- Jobs: `https://www.ncss.cn/student/jobs/<id>/detail.html` (what lists
  print; `job.ncss.cn` and the provinces' and schools' sites under
  `ncss.cn` too), `jobs/<id>`, or the id (`MprVzHq4QAhDGX1XiSpy1k`).
- Companies: `https://www.ncss.cn/student/jobs/<id>/corp.html` or the id
  of 32 hex digits. Companies without a page of their own (many jobs of
  schools and provinces) have no id.
- Job fairs: `https://www.ncss.cn/student/jobfair/fairdetails.html?fairId=<id>`
  (what `fairs` prints), or the id. Fairs held on other sites link there
  and have no jobs here.
- `#N` works after any list.

## Commands

### search

`search WORDS` searches every job of the site, 20 a page, in the site's
order. `--filter full-time|part-time|intern` keeps one job type,
`--filter announcement` the employers' hiring announcements
(公告, several jobs in one). `jobs` narrows further.

### jobs

`jobs [WORDS]` is the search with the 职位信息 page's filters; without
words it lists every job the filters let through.

- `--city`: a province (北京, 广东), a city (杭州) or a district (海淀区)
  by name, or a code from `areas`; all of China by default.
- `--type full-time|part-time|intern` (全职, 兼职, 实习 too).
- `--kind job|announcement`: single jobs, or announcements (公告).
- `--degree any|college|bachelor|master|phd`: the education asked for,
  at least (不限, 专科, 本科, 硕士, 博士 too).
- `--salary 0-2k|2-5k|5-10k|10-15k|15-25k|25-50k|50k+|negotiable`:
  monthly pay.
- `--category`: a job category (计算机, 销售类 …) or its code;
  `categories` lists them.
- `--industry`: an industry (计算机软件, 银行 …) or its code, several (up
  to ten) for any of them; `industries` lists them.
- `--property state|joint-stock|private|listed|hmt|joint-venture|foreign|public|other`:
  who owns the company (国企, 民企, 外企, 事业单位 too).
- `--source platform|schools|boss|chinahr|liepin|qlrc|51job`: where the
  job was posted: on the platform, on the provinces' and schools' sites,
  or on a partner site (BOSS直聘, 中华英才网, 猎聘, 全联人才在线, 前程无忧).
- `--key-field`: companies of key fields (重点领域); `--featured`: the
  platform's featured companies (精选企业).

    atp ncss jobs java --city 杭州 -n 40
    atp ncss jobs --type intern --city 北京 --category 计算机

### areas, categories, industries

`areas` lists the provinces with their codes, `areas NAME` the cities and
districts whose name contains it (with their province). `categories` and
`industries` list the codes of `--category` and `--industry`.

### read

A job's page: the whole description, the pay as shown, the degree, the
openings, the majors wanted, the place, the job type, where it was posted
(`extra.source`) and the company with its industry, fields, ownership,
size, website and address. Job pages carry no date with a year; lists
give the times.

### user, user-posts

`user COMPANY` reads a company's page: name, logo, introduction,
ownership, industry, fields, size, website and address. `user-posts
COMPANY` lists the jobs it has open on the platform, 20 a page.

### fairs

`fairs` lists the site's job fairs, 16 a page, in the site's order: each
with its dates (`extra.starts`, `extra.ends`), sponsor, state (`open`,
`upcoming`, `ended`) and, when the site counts them, its companies, jobs
and openings. `--list online` (the default) are the site's own online
fairs (中心站点招聘会), `--list local` the fairs held around the country
(各地招聘会), `--list 24365` those of the 24365 campaign (全国网络联合招聘),
which come in one page. Fairs held on other sites (`extra.elsewhere`)
link there.

### fair

`fair FAIR [WORDS]` lists the jobs of one of the site's fairs, 10 a page,
with the filters of `jobs` the fair's page has: `--city`, `--type`,
`--salary`, `--category`, `--industry`, `--property`.

    atp ncss fairs
    atp ncss fair '#1' 软件 --city 重庆

### fair-companies

`fair-companies FAIR [NAME]` lists the companies of a fair, 16 a page,
those whose name contains `NAME` when given; `user-posts` lists one's
jobs.

### favorite

Saves a job (收藏), as its page's button does. Needs a login. Taking a
job out again (`unfavorite`) is not offered: the site does it by the id
of the saved entry, which ATP cannot read; do it on the site.

### apply

`apply JOB` applies to one job with the résumé you keep on the site, as
its page's 投递简历 does. It shows the job (title, company, pay, place,
where it was posted, link) and asks first; `-y` skips the question.

- Jobs posted on the platform: the site first says whether the employer
  takes applications on its own site; ATP then reports that site instead
  of applying.
- Jobs of schools, provinces and partner sites: the site records the
  application and names the site it goes on at, which ATP prints; finish
  there.

One job per command, by design. Needs a login.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in
`extra`, where the card or page tells them:

- `company` (the `author` is the company, with its page when it has one);
- `salary` as shown (`8-14K/月`, `10k-15k`, `面议`), with `salary_min` and
  `salary_max` per month, `salary_currency` (CNY); `面议` has no figures;
- `city` (lists name the province or the city, pages the city and
  district), `degree`, `job_type` (`full-time`, `part-time`, `intern`,
  when the page or the filter says).

NCSS's own: `source` (where the job was posted: 国家大学生就业服务平台, or
the school, province or partner site) and `source_type` (`school`,
`province`, `job site`), `major` (the majors wanted), `openings`,
`announcement` (an employer's announcement of several jobs),
`key_field` (重点领域), `featured` (精选企业), `company_size`,
`company_property` and, on pages, `company_industry`. Lists date jobs by
their publication and update; the company's labels (五险一金 …) are tags.

Fairs (`kind: fair`) carry `starts`, `ends`, `sponsor`, `status`,
`companies`, `jobs`, `openings` and `elsewhere`.

## Limits

- No list of your saved jobs or applications, no `unfavorite`: the pages
  of your own lists were not seen without an account.
- No résumé editing, no applying to several jobs at once (the site's
  批量投递), no registering for fairs, no interview reviews (面试经验).
- The provinces' and schools' own job sites are not read; their jobs
  appear here as the platform shares them, applied to on their sites.
- Without a login the job search stops after ten pages.
