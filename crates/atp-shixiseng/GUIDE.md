Shixiseng (实习僧, shixiseng.com), where Chinese students look for
internships and campus jobs, through the requests its own pages make: the
search page's job search with its filters (city, days a week, months,
degree, daily pay, return offers, publication date, well-known
companies), campus jobs, job and company pages, a company's jobs and the
company search; with a login, saved jobs, your applications and applying
to one job at a time.

## Logging in

### WeChat mini-program code

`atp shixiseng login` shows the code of the site's own login panel: not a
QR code but a code of Shixiseng's WeChat mini program (实习校招), saved as
an image (`login-qr.jpg` in the cache folder, whose path is printed) for
you to open and scan with WeChat. The mini program asks to confirm; ATP
then keeps the session cookie the site hands out. ATP asks every 20
seconds or so, as the page does, and gives up after two minutes. A WeChat
account without a Shixiseng account is led to the site's sign-up, which
ATP does not do. This login follows the site's current scripts and has
not been tried with a real account.

### Browser or cookie

The site keeps its session in the `SXS_XSESSION_ID` cookie of
shixiseng.com (with `SXS_XSESSION_ID_EXP` beside it).

- `atp shixiseng login --browser` reads the cookies of a browser logged
  in at shixiseng.com.
- `atp shixiseng login --cookie 'SXS_XSESSION_ID=…; SXS_XSESSION_ID_EXP=…'`:
  a `Cookie` header holding them (developer tools, Application, Cookies,
  `https://www.shixiseng.com`); the site's other cookies may come along.
- `ATP_SHIXISENG_COOKIE='SXS_XSESSION_ID=…'` passes them for one run;
  they are never saved.

The site gives every visitor a session under the same cookie name, so a
login is checked by asking the site for the account (`whoami`); a browser
that only visited the site is refused. Without a login ATP forgets the
visitor session the site hands out. The site's texted-code and password
logins sit behind Tencent's captcha and are not offered.

### Without an account

`search`, `jobs`, `read`, `user` and `user-posts` work without logging
in.

### With an account

`whoami` (id, name and avatar; the phone number and login name are left
out), `favorites`, `favorite` / `unfavorite`, `applied` and `apply`. None
of them has been tried with a real account: they follow the site's
current scripts, and the shapes of their answers are read defensively.

### How long it lasts

How long the site keeps a login is not published. When it has ended,
commands fail with `not_authenticated` (the site's code 101, `请先登录`)
and the hint to log in again.

### Risk control

- ATP keeps at least 2 s between requests and waits 2 to 5 s more before
  each save and application. The site does not publish its limits.
- The site sits behind 知道创宇's 加速乐 firewall. When it doubts a
  request it answers with a script for the browser to run (HTTP 521, the
  `__jsl_clearance_s` cookie) instead of the data: ATP reports
  `verification_required` with the page and, on a terminal, opens it in
  your browser with the session's cookies, then retries once with the
  cookies it set. ATP never runs that script itself, and does not retry
  such answers on its own. Not met while ATP was built.
- Applying sends your résumé to the employer; one job per command, after
  showing it and asking.

### Environment

`ATP_SHIXISENG_COOKIE`: the session's cookies for one run.

## References

- Jobs: their id (`inn_l1azdlkp0wof`), or a link holding it:
  `https://www.shixiseng.com/intern/<id>` (what lists print), shared
  links with `?utm_…` too.
- Companies: their id (`com_qn76ufrrerod`), or a link holding it:
  `https://www.shixiseng.com/com/<id>` (what lists print).
- `#N` works after any list.

## Commands

### search

`search WORDS` searches internships all over China, 20 a page, in the
site's order, or with `--sort new` the newest first. `--filter campus`
searches campus jobs (校招) instead, which the site lists in its own order
and narrows by city only (`jobs --kind campus --city`). `-t user`
searches companies by name. `jobs` narrows internships further.

The site writes the titles, pay, days a week, months and company sizes of
its internship cards with characters of a private font, which its page
draws with a font of its own. ATP reads which character each one stands
for from that font's glyph names (no picture is read or recognized) and
keeps the map a few hours; when the cards stop reading right against the
plain pay figures they also carry, it reads the font again.

### jobs

`jobs [WORDS]` is the search with the site's filters; without words it
lists every job they let through.

- `--kind intern|campus` (实习, 校招 too): internships by default. The
  campus search takes words and `--city` only.
- `--city`: a city as the site writes it (北京, 上海, 深圳 …); all of
  China (全国) by default.
- `--days 1-6`: the site's 每周几天 filter (6 for six or more); the jobs
  it kept when tried asked for that many days a week or more.
- `--months 1|2|3|3+`: how long the internship lasts.
- `--degree college|bachelor|master|phd` (大专, 本科, 硕士, 博士 too).
- `--pay 0-100|100-150|150-200|200-300|300+`: daily pay in yuan.
- `--offer yes|no|maybe`: whether the internship may turn into a job
  (提供转正, 不提供转正, 面议).
- `--published day|week|month`: published within that time.
- `--top`: well-known companies only (知名企业).
- `--new`: the newest first instead of the site's order.

Filters combine; several together often leave nothing.

    atp shixiseng jobs 数据分析 --city 北京 --days 4 --pay 200-300
    atp shixiseng jobs --kind campus 算法 --city 杭州

### read

A job's page: the whole description, the pay as shown, city and
address, days a week and months (internships), degree, the deadline, the
category, perks and skills (as tags), who it is for (`audience`: 应届生 …),
whether it may turn into a job (`return_offer`), how many applied
(`applicants`), the company with its industry, size and funding, and with
a login whether you applied or saved it. Jobs gathered from other sites
name where they came from (`source`) and some an application page of
their own (`apply_url`).

### user, user-posts

`user COMPANY` reads a company's page: name, logo, introduction, city,
address, size, industry, funding, ownership, registration (legal name,
type, founding date, capital), website, tags and its open internships and
campus jobs (`stats.posts`). `user-posts COMPANY` lists its jobs,
internships and campus jobs together, 10 a page. The company's phone
number is left out.

### favorites, favorite, unfavorite

Your saved jobs (职位收藏), internships and campus jobs in one list, 10 a
page, each with `status` (`normal`, or the site's word for a job no
longer taking applications); and saving a job or taking it out. Saving
reads the job's page first, for the kind of job the site files it under.
Needs a login.

### applied

Your applications (投递记录), 10 a page, in the site's order: the job,
its company and city, when you applied, and the stage
(`extra.status`: `delivered`, `checked`, `undeter`, `written`, `notify`,
`reject`, `employed`, with the site's words in `extra.status_label`).
`--stage viewed|screening|test|interview|rejected|hired` keeps one tab of
the site's list (被查看, 初筛, 笔试, 面试, 不合适, 录用). The messages
companies send with a stage are not read. Needs a login.

### apply

`apply JOB` applies to one job with your résumé on the site, as the job
page's 投递简历 dialog does. It shows the job (title, company, pay, city,
link), the résumé and what goes with it, and asks first; `-y` skips the
question.

- `--resume ID`: the résumé, by the group id the summary shows, when the
  account has several the job takes.
- `--days 1-7`, `--start 1w|2w|1m|3m` (when you can start) and `--stay
  0-3m|3-6m|6m+` (how long you can stay): what the dialog asks; by
  default what the site fills in.

Before sending, ATP reports instead of applying when you applied
already, the job is closed, or it is applied to on a page of its own.
Jobs the site gathered from other sites are sent the way the dialog
sends them. One job per command, by design.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in
`extra`:

- `company` (the `author` is the company, with its page);
- `salary` as shown (`100-150/天` for internships, `6k-8k/月` for campus
  jobs, `薪资面议`), with `salary_min` and `salary_max` per
  `salary_period` (`day` or `month`) and `salary_currency` (CNY); 面议 has
  no figures;
- `city`, `degree`, `job_type` (`intern`, or `full-time` for campus jobs)
  and, on job pages, `deadline`.

Shixiseng's own: `recruit_type` (`intern`, `campus`, `social`),
`days_per_week` and `months` (internships), `category`, `address`,
`audience`, `return_offer`, `applicants`, `applied` and `saved` (with a
login), `source` and `apply_url`, `company_industry`, `company_size` and
`company_funding`. Search cards carry no publication time; job pages
their last refresh (`updated_at`).

## Limits

- No filter by district or industry (the search page has none by
  industry; its districts need another lookup, not done), and the campus
  search narrows by words and city only.
- No applying to several jobs at once (批量投递), no withdrawing an
  application, no résumé editing, no following companies, no messages
  from companies.
- The messages behind an application's stages (`deliver/details`) are not
  read: they may carry recruiters' contact details.
- Phone numbers, e-mail addresses for applications and the Q&A and
  advertising blocks of job pages are dropped from every payload, `raw`
  included.
