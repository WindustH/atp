Zhilian Zhaopin (智联招聘, zhaopin.com), one of China's large job sites,
through the requests its own pages make: the job search of www.zhaopin.com
with the site's filters (city, monthly pay, experience, education, job
type), whose cards bring each job's whole description, job and company
pages of the mobile site, and with a login the account and applying to
one job at a time.

## Logging in

### Browser or cookie

The site keeps its login in the `at` and `rt` cookies of zhaopin.com;
its scripts also send them as `x-zp-at` and `x-zp-rt` headers, and ATP
does the same.

- `atp zhilian login --browser` reads the cookies of a browser logged in
  at zhaopin.com.
- `atp zhilian login --cookie 'at=…; rt=…'`: a `Cookie` header with both
  (developer tools, Application, Cookies, `https://www.zhaopin.com`); the
  other cookies of the site may come along.
- `ATP_ZHILIAN_COOKIE='at=…; rt=…'` passes them for one run; they are
  never saved.

The site's own logins (texted codes, passwords, both behind its Geetest
captcha) are not offered, nor a QR login: the site's scripts ATP could
read have none.

### Without an account

`search`, `jobs`, `cities`, `read` and `user` work without logging in.

### With an account

`whoami` (the account's id, name, avatar and how many résumés it has;
the phone number and e-mail address are left out by the site) and
`apply`. Neither has been tried with a real account: they follow the
site's current scripts, and the shapes of their answers are read
defensively.

### How long it lasts

How long the site keeps a login is not published. When it has ended,
commands fail with `not_authenticated` and the hint to log in again.

### Risk control

- ATP keeps at least 3 s between requests and waits 2 to 5 s more before
  applying. The site does not publish its limits.
- www.zhaopin.com and m.zhaopin.com sit behind Tencent EdgeOne, which
  answers requests it doubts with a check instead of the page: a
  checkbox ("Security Verification") or a script for the browser to run.
  The job and company pages ATP reads come from m.zhaopin.com and are
  often checked so. ATP reports `verification_required` with the page
  and, on a terminal, opens it in your browser with the session's
  cookies, then retries once with the cookies the check set. It never
  passes checks itself.
- The search (fe-api.zhaopin.com) is not behind EdgeOne. When it asks for
  a check (`isVerification`), ATP reads the mobile search page instead,
  for the words and the city alone; with other filters it reports
  `verification_required`, which a browser cannot clear for it.
- The site may ask for its security certification (code 2024,
  `verification_required`) and, before some applications, a face check
  in its app (status 2021, reported, not opened).
- Applying sends a résumé to the employer; one job per command, after
  showing it and asking.

### Environment

`ATP_ZHILIAN_COOKIE`: the session's cookies for one run.

## References

- Jobs: their number (`CC000638920J40889851614`), or a link holding it:
  `https://m.zhaopin.com/jobs/<number>.htm` (what lists print),
  `https://jobs.zhaopin.com/<number>.htm`,
  `https://www.zhaopin.com/jobdetail/<number>.htm`.
- Companies: their number (`CZ000638920`, `CZL1337269640`), or a link
  holding it: `https://m.zhaopin.com/company/<number>.htm` (what lists
  print), `https://company.zhaopin.com/<number>.htm`,
  `https://www.zhaopin.com/companydetail/<number>.htm`.
- `#N` works after any list.

## Commands

### search

`search WORDS` searches jobs all over China, 20 a page, in the site's
order. `--filter full-time|part-time|intern|campus` keeps one kind of
job (校园 is campus hiring of new graduates). `jobs` narrows further.
Each card brings the job's whole description (`text`), its skills and
benefits (as tags) and the company's size, industry and ownership.

### jobs

`jobs [WORDS]` is the search with the site's filters; without words it
lists every job they let through.

- `--city`: a city's name (北京, 杭州, 阿拉尔市; 市 may be left out) or its
  code; `cities` lists them. All of China (489) by default.
- `--salary 15-25`: monthly pay in thousands of yuan (`15k-25k` too);
  jobs whose pay overlaps the range.
- `--experience 0-1|1-3|3-5|5-10|10+`: years of work asked for.
- `--degree phd|master|bachelor|college|high-school` (博士, 硕士, 本科,
  大专, 高中 too).
- `--type full-time|part-time|intern|campus` (全职, 兼职, 实习, 校园 too).

    atp zhilian jobs rust --city 北京 -n 40
    atp zhilian jobs 产品经理 --city 杭州 --salary 15-25 --experience 3-5

### cities

`cities` lists the hot cities with their codes (全国 first), `cities
NAME` every city whose name contains it.

### read

A job's mobile page: the whole description, the pay as shown, the city,
district and address, experience, degree, openings, the category, skills
and benefits (as tags), the company with its industry, size and the line
under its name, and how applications are taken (`apply_url` for jobs
applied to on their own page, `company_ats` for jobs passing the résumé
to the company's own hiring system, `closed`). The page is often behind
the firewall's check (see Risk control); the search's cards carry the
same description.

### user

`user COMPANY` reads a company's mobile page: name, logo, introduction,
city, size, industry, ownership, funding, the line under its name and the
number of open jobs (`stats.posts`). The page is behind the firewall's
check for ATP's requests; it is read by the keys of the company data the
job pages carry and has not been seen whole.

### apply

`apply JOB` applies to one job with your résumé on the site, as the job
page's 投递 does. It shows the job (title, company, pay, city, link) and
the résumé and asks first; `-y` skips the question. An account with
several résumés picks one with `--resume NUMBER` (`whoami --raw` lists
them). Before sending, ATP reports instead of applying when the job is
closed or already applied to, is a campus job taken on the company's
campus site, has an application page of its own, or passes the résumé
to the company's own hiring system (which the site asks you to agree to
on the site); and when the site's check before applying stops it, with
its message. One job per command, by design.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in
`extra`:

- `company` (the `author` is the company, with its page);
- `salary` as shown (`1.5-2.5万·14薪`, `8000-12000元`, `150-250元/天`,
  `面议`), with `salary_min` and `salary_max` per `salary_period` (`month`,
  `day` for internships), `salary_currency` (CNY) and `salary_months`;
  `面议` has no figures;
- `city`, `experience`, `degree` and `job_type` (`full-time` for 全职 and
  校园, `part-time`, `intern`).

Zhilian's own: `district`, `address` (pages), `work_type` (the site's
word: 全职, 校园, 实习, 兼职/临时), `category`, `openings`, `applied` (with
a login), `company_size`, `company_industry`, `company_property` and, on
pages, `apply_url`, `company_ats` and `closed`. Cards are dated by their
publication.

## Limits

- No list of your applications, of who viewed your résumé or of your
  messages: the site's scripts ATP could read do not show how their
  pages ask for them. Chats with recruiters (a socket protocol) are not
  offered.
- No company's job list (`user-posts`), no filters by company size,
  ownership, industry or publication date: their codes could not be
  checked.
- No résumé editing, no saving jobs, no applying to several jobs at once.
- Recruiters (names, titles, avatars, activity) and the phone button's
  state are dropped from every payload, `raw` included.
