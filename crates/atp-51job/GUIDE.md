51job (前程无忧, 51job.com), one of China's large job sites, read
without an account: the job search with the site's filters (city,
monthly pay, experience, education, the company's ownership and size,
job type), whose cards bring each job's whole description, and job and
company pages.

51job answers only a real browser: Aliyun's WAF in front of its search
API runs scripts that plain requests cannot. ATP never runs or copies
those scripts. It starts Chrome, Chromium, Edge or Brave on your
machine, without a window, lets the site's own search page load in it
and asks that page for the results, the way the page itself asks
(`atp guide login` explains the browser ATP uses for human checks). The
first request of each command takes a few seconds more while the browser
starts; the browser ends with the command.

## Logging in

There is nothing to log in to: every command works without an account,
and `atp 51job status` always reports "not logged in". The site keeps a
login in its pages' storage and sends it as headers to its account API
(cupid.51job.com); the requests its job pages make to save a job or
apply were not in the scripts ATP could read, so neither a login nor
saved jobs, applications or applying are offered.

### The browser

- Chrome, Chromium, Edge or Brave must be installed; `ATP_BROWSER`
  names one by its path or command.
- It runs on a profile of 51job's own,
  `<cache>/all-the-platforms/51job/browser`, kept between runs so the
  site's cookies and storage (its device ids, the WAF's cookies) last;
  `atp 51job logout` deletes it. Two atp runs at once: the second uses
  an empty profile for its run.
- It goes through ATP's proxy (`--proxy`, `ATP_PROXY`, `HTTPS_PROXY`).
- The browser is used as it is: no other user agent, nothing hidden or
  added to the site's pages.

### Risk control

- ATP keeps at least 5 s between requests, page loads and searches
  alike. The site does not publish its limits.
- When this was written, the search page and its API let ATP's browser
  through. The job and company pages (jobs.51job.com) answered it with
  the WAF's slider (滑动验证) each time they were tried, and plain
  requests with the WAF's script check. ATP reports such a check as
  `verification_required` with the page. On a terminal it opens the page
  in a window of the same browser, on the same profile, for you to drag
  the slider yourself; press Enter and the command runs again. Whether
  the pages then open for the browser without a window has not been
  seen. ATP never drags the slider. In pipes, scripts, AI agents and
  with `--no-verify`, run the command once on a terminal to pass it.

### Environment

`ATP_BROWSER`: the browser to use. No credentials are read.

## References

- Jobs: their number (`166040584`), or a link holding it: the job page
  `https://jobs.51job.com/<place>/<number>.html` (what lists print; the
  place is a word like `xian-ytq`, or `all`), a company career site's
  `…/detail.html?jobid=<number>` (the `apply_url` of such jobs) or any
  link with `jobid=<number>`.
- Companies: their code (`VjlTMABlAD4CYFAwXTE`, the `id` of a job's
  `author`), or their page `https://jobs.51job.com/all/co<code>.html`
  (the author's `url`).
- `#N` works after any list of jobs; companies take their code or link.

## Commands

### search

`search WORDS` searches jobs all over China, 20 a page, in the site's
order. `--sort newest|salary|active` takes the search page's other
orders (最新优先, 薪资优先, 活跃职位优先: recruiters active of late);
`--filter full-time|part-time|intern` keeps one kind of job. Each card
brings the job's whole description (`text`), its skills and benefits
(as tags) and the company's ownership, size and industry. `jobs`
narrows further.

### jobs

`jobs [WORDS]` is the search with the site's filters, with the codes of
its search page; without words it lists every job they let through.

- `--city`: a city or province by name (北京, 杭州, 广东省; 市 and 省 may
  be left out), by Latin name (`hangzhou`; names two places share, such
  as `suzhou`, ask for the Chinese name) or by code; `cities` lists them.
  All of China (000000) by default.
- `--salary 0-8k|8-10k|10-15k|15-20k|20-30k|30-40k|40-50k|50k+`: monthly
  pay in thousands of yuan; the site keeps jobs whose pay overlaps it.
- `--experience student|1-3|3-5|5-10|10+|none` (在校生/应届生 … 无需经验);
  jobs asking for less count too (2年及以上 is in 3-5).
- `--degree middle-school|high-school|college|bachelor|master|phd|none`
  (初中, 高中, 大专, 本科, 硕士, 博士 too).
- `--company-type state|western|foreign|listed|joint-venture|private|representative|government|institution|nonprofit|startup`
  (国企, 外资（欧美）, 外资（非欧美）, 已上市, 合资, 民营, 外企代表处, 政府机关,
  事业单位, 非营利组织, 创业公司).
- `--size 0-50|50-150|150-500|500-1000|1000-5000|5000-10000|10000+`:
  people at the company.
- `--type full-time|part-time|intern` (全职, 兼职, 实习), as the site
  classes jobs: `intern` keeps internships even where their card says
  全职.
- `--sort`, as for `search`.

City, pay, experience, degree, ownership, size, `--type intern` and
`--sort newest` were each seen keeping the jobs they name; the other
job types and orders come from the site's search page and were not
tried.

    atp 51job jobs rust --city 北京 -n 40
    atp 51job jobs 产品经理 --city 杭州 --salary 15-20k --experience 3-5

### cities

`cities` lists 全国 and the hot cities with their codes, `cities NAME`
every place whose name or Latin name contains it.

### read

A job's page on jobs.51job.com: the title, pay, city and district,
experience, degree, openings, the description, benefits (as tags), the
category (职能类别), the address and the company with its ownership, size
and industry. The page has shown ATP the WAF's slider (see Risk
control); what `read` reads follows the page's markup as other tools
read it and has not been seen working. The search's cards carry the same
description.

### user

`user COMPANY` reads a company's page: name, logo, ownership, size,
industry and introduction. Like `read`, it has met the slider and has not
been seen working.

### user-posts

`user-posts COMPANY` lists the jobs a company's page shows, as one page.
Like `read`, it has met the slider and has not been seen working.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in
`extra`:

- `company` (the `author` is the company, with its page);
- `salary` as shown (`1.5-2万·14薪`, `9千-1.8万`, `25-40万/年`,
  `200元/天`, `面议`), with `salary_min` and `salary_max` per
  `salary_period` (`month`; `year` and `day` where the pay says so),
  `salary_currency` (CNY) and `salary_months`;
- `city`, `experience`, `degree`, `job_type` (`full-time`, `part-time`,
  `intern`, by the card's 全职, 兼职, 实习) and `remote` (jobs marked as
  remote).

51job's own: `district`, `landmark`, `work_type` (the site's word),
`majors`, `applied` and `closed` (as the site marks them),
`company_full_name`, `company_type`, `company_size`,
`company_industry`, and `apply_url` for jobs of companies with a career
site of their own on 51job (`<company>.51job.com`). On job pages also
`openings`, `category` and `address`. Cards are dated by their
publication (`created_at`) and their last change (`updated_at`).

## Limits

- No login, saved jobs, applications or applying (see Logging in).
- No hot list: the site's lists of hot jobs answered ATP with no jobs,
  and the parameters its pages send were not in the scripts ATP could
  read.
- No filters by industry, job function, district, metro line or
  publication date.
- `read`, `user` and `user-posts` have not been seen working (see Risk
  control).
- Recruiters (names, titles, pictures, whether they are online) and the
  state of the chat and phone buttons are dropped from every payload,
  `raw` included.
- A Chromium-family browser is needed; without one every command fails
  with `internal_error`.
