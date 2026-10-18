BOSS Zhipin (BOSS直聘, zhipin.com), the job site where seekers chat with
recruiters directly: the hot jobs of its city pages, the job search of
its 职位 page with the site's filters (city, monthly pay, experience,
education, industry, company size and funding stage, full-time or
part-time), job pages and company pages. With a login: the jobs you
marked 感兴趣 (saved jobs), the jobs you chatted about, your
applications, interviews, the companies you follow, your chats with
recruiters, and applying to one job at a time by greeting its recruiter.

The site's job search and job pages answer only requests carrying a
cookie its own security script computes in the page (`__zp_stoken__`, a
scrambled program that reads the browser it runs in). ATP never runs,
copies or computes that script. It starts Chrome, Chromium, Edge or
Brave on your machine, without a window, lets the site's search page or
job page load there with its scripts, and reads what the page asked for
or shows (`atp guide login` explains the browser ATP uses for human
checks). See Risk control for how the site has answered that browser.

## Logging in

### QR code

`atp boss login` shows the QR code of the site's login page. Scan it with
the BOSS app (BOSS直聘 APP) and confirm on the phone; the site then sets
the session's cookies (`wt2`, `wbg`, `zp_at` and `bst`), which ATP keeps.
ATP waits for the scan and the confirmation as the page does (each call
waits up to half a minute for news) and gives up after three minutes.
This login has not been tried with a real account.

### Browser or cookie

- `atp boss login --browser` reads the cookies of a browser logged in at
  zhipin.com.
- `atp boss login --cookie '…'`: a `Cookie` header copied from the
  browser's developer tools (any request to `www.zhipin.com`).
- `ATP_BOSS_COOKIE='…'` passes the cookies for one run; they are never
  saved.

The session needs the `wt2` and `zp_at` cookies. Texted codes and
WeChat logins are not offered. The account must be in its job seeker
role (牛人身份): an account the site treats as a recruiter gets code 24,
which ATP reports as `permission_denied`; switch roles in the BOSS app.

### Without an account

`hot`, `cities` and `industries`; `search`, `jobs`, `read`, `user` and
`user-posts` through the browser (see Risk control). The site shows a
search's first 15 jobs to visitors; its search page asks for further
pages only with a login.

### With an account

`whoami`, `favorites`, `favorite` / `unfavorite`, `following`,
`follow` / `unfollow`, `chatted`, `applied`, `interviews`, `apply`,
`chats` and `messages`, and the search's pages after the first. None of
them has been tried with a real account: they follow the requests of the
site's current pages, and the shapes of their answers are read
defensively. The account's lists and actions go over plain requests with
the session's cookies (but never `__zp_stoken__` or the other cookies of
the security script); when the site answers one with its environment
check (code 37), ATP makes the same request from a page of the site in
the browser. The search's further pages are always asked from the search
page in the browser.

### How long it lasts

How long the site keeps a login is not published. When it ends,
commands fail with `not_authenticated` (the site's codes 7 and 1011, or
120 to 122 for a refused `zp_token`, which ATP renews once as the site's
pages do) and the hint to log in again.

### The browser

- Chrome, Chromium, Edge or Brave must be installed; `ATP_BROWSER`
  names one by its path or command.
- It runs on a profile of BOSS Zhipin's own,
  `<cache>/all-the-platforms/boss/browser`, kept between runs so the
  site's cookies and storage last; `atp boss logout` deletes it with the
  session. Two atp runs at once: the second uses an empty profile.
- It goes through ATP's proxy (`--proxy`, `ATP_PROXY`, `HTTPS_PROXY`).
- The browser is used as it is: it tells pages it is HeadlessChrome and
  automated (`navigator.webdriver`), as Chrome without a window does;
  ATP hides nothing and adds nothing to the site's pages.

### Risk control

- When this was written (October 2026), the site's security check
  turned the browser without a window away every time: the search
  page's request got the environment check (code 37, 您的环境存在异常),
  the site's security page (请稍候) ran its script and sent the browser
  back to the search, which got the check again, until the site showed
  its home page. ATP reports this as `verification_required` with the
  page after about 30 s. The check most likely reads the browser's
  HeadlessChrome name and automation flag, which ATP does not disguise.
  So `search`, `jobs`, `read`, `user` and `user-posts` have not been
  seen working.
- On a terminal (without `--no-verify`), ATP then opens the page in a
  window of the same browser, on the same profile, where the site's
  scripts run as in any browser; press Enter and the command runs again
  without a window. Whether the site then lets the windowless browser
  through, with the cookie its script set in the window, has not been
  seen.
- The site's captcha page (安全验证, code 5012) is reported the same
  way; ATP never solves it.
- ATP keeps at least 5 s between requests and page loads. Too many
  requests get code 9 (`rate_limited`); ATP does not retry, wait a
  while. Saving, following and applying wait 2 to 5 s more.
- The site limits how many recruiters an account greets a day; the
  numbers are not published. Greeting in bulk is what gets accounts
  restricted, so `apply` greets one job's recruiter per command, after
  asking.

### Environment

`ATP_BOSS_COOKIE`: the session's cookies for one run. `ATP_BROWSER`:
the browser to use.

## References

- Jobs: their id (`af620b46e682c71f0nZ409q8FVNV`), or the job page
  `https://www.zhipin.com/job_detail/<id>.html` (`m.zhipin.com` links
  too). Lists print the link as the site's own cards make it, with
  `?securityId=…&lid=…`, the ids actions on the job take; links copied
  from the site carry them too.
- Companies: their id (`ed38088b0f836f9e1Xd80t64EQ~~`, the `id` of a
  job's `author`), or the company page
  `https://www.zhipin.com/gongsi/<id>.html` (or `/gongsi/job/<id>.html`).
- Chats: the number `chats` lists.
- `#N` works after any list.

## Commands

### hot

`hot` lists the 热招职位 box of the site's home page, for the city the
site places you in by your address; `hot -c CITY` that of a city's own
page (北京, 上海, 广州, 深圳, 杭州 … the 25 cities the home page links;
a name, pinyin or code). About 80 jobs under nine kinds of work (销售,
服务业, 技术 …, in `category`), each with its city, experience, degree,
keywords and the company with its industry and funding stage. The pages
answer plain requests, without the browser and without the session's
cookies; their cards carry no pay. Seen working.

### search

`search WORDS` searches jobs all over China (全国), 15 a page, in the
site's order; `--filter full-time|part-time` keeps one kind of job.
`jobs` narrows further. Read through the browser (see Risk control).

### jobs

`jobs [WORDS]` is the search with the filters of the site's 职位 page,
as their codes come from the site's own data; without words it lists
what the page recommends for the city and filters (one page).

- `--city`: a city by name (北京, 杭州; 市 may be left out), pinyin
  (`hangzhou`; pinyin two cities share, such as `suzhou`, asks for the
  Chinese name) or code; `cities` lists them. All of China by default.
- `--salary 0-3k|3-5k|5-10k|10-20k|20-50k|50k+`: monthly pay in thousands
  of yuan.
- `--experience student|graduate|none|0-1|1-3|3-5|5-10|10+` (在校生,
  应届生, 经验不限, 1年以内 …).
- `--degree middle-school|technical|high-school|college|bachelor|master|phd`
  (初中及以下, 中专/中技, 高中, 大专, 本科, 硕士, 博士 too).
- `--industry`: an industry by name (人工智能, 游戏, 计算机软件) or code;
  `industries` lists them.
- `--size 0-20|20-99|100-499|500-999|1000-9999|10000+`: people at the
  company.
- `--stage unfunded|angel|a|b|c|d+|listed|self-funded` (未融资, 天使轮,
  A轮, B轮, C轮, D轮及以上, 已上市, 不需要融资).
- `--type full-time|part-time` (全职, 兼职).

The filters go into the search page's address under the names the page
reads; whether the site keeps the jobs they name has not been seen (see
Risk control). Without a login the site shows the first 15 jobs; with
one, further pages are asked from the search page as its scroll does.

    atp boss jobs rust --city 北京
    atp boss jobs 产品经理 --city 杭州 --salary 20-50k --experience 3-5

### cities

`cities` lists 全国 and the hot cities with their codes, `cities NAME`
every city whose name or pinyin contains it. The table is the site's own
(374 cities).

### industries

The industries the site files companies under, with their codes and
groups, for `jobs --industry`.

### read

A job's page: the title, pay, city, experience, degree, the whole
description, skills and benefits (as tags), the address and the company
with its industry, funding stage and size. Job pages first pass the
site's security page, which has turned the browser away (see Risk
control); what `read` reads follows the page's markup as other tools
read it and has not been seen working.

### user

`user COMPANY` reads a company's page: name, logo, funding stage, size,
industry, introduction and address. Like `read`, it has not been seen
working.

### user-posts

`user-posts COMPANY` lists the jobs of the company's page of jobs, as one
page. Like `read`, it has not been seen working.

### favorites

The jobs you marked 感兴趣, the site's saved jobs (needs a login).

### favorite, unfavorite

Mark one job 感兴趣 or take the mark off, as the job's page and your
list's 取消感兴趣 button do (needs a login). The ids the site wants come
with the links lists print (`favorite '#3'`); for a bare id or a plain
link ATP reads the job's page first, through the browser (see Risk
control).

### following

The companies you follow (公司收藏), with their industry, funding stage,
size and benefits (needs a login). The site shows no one else's.

### follow, unfollow

Follow a company (收藏) or leave it, as the 收藏 button of your list of
companies does (needs a login).

### chatted

The jobs whose recruiters you chatted with (沟通过; needs a login).

### applied

Your applications (已投递): the jobs you sent a résumé for (needs a
login).

### interviews

The interviews recruiters invited you to (面试): the job, the company,
when, where and their state (needs a login).

### apply

On BOSS Zhipin one applies by greeting a job's recruiter (打招呼), which
opens a chat with them; the site sends your account's default greeting,
set in the BOSS app. `apply JOB` does that for one job, as the job page's
立即沟通 button does: it reads the job's page (through the browser, see
Risk control), shows the job and asks first (`-y` skips the question).
ATP adds no text of its own, greets one job per command and does not
send résumés (needs a login). The chat then shows in `chats`.

    atp boss apply https://www.zhipin.com/job_detail/af620b46e682c71f0nZ409q8FVNV.html
    atp boss apply '#3' -y

### chats, messages

`chats` lists your chats with recruiters, named by company and job (with
the job's link and the last message's time); `messages CHAT` a chat's
messages, 20 a page, each marked `sent` or `received` (needs a login).
Recruiters' names and pictures are left out. Phone numbers and WeChat
ids inside messages are replaced by `[contact]`, handed-over phone
numbers show as `[contact exchange]`, voice messages as `[voice]` and
other cards (résumés, jobs, interviews) as `[card N]`. Sending is not
offered.

## Job posts

Jobs (`kind: job`; `application` in `applied`) carry the job keys every
job platform uses in `extra`:

- `company` (the `author` is the company, with its page);
- `salary` as shown (`15-25K·13薪`, `150-200元/天`), with `salary_min`
  and `salary_max` per `salary_period`, `salary_currency` (CNY) and
  `salary_months`. Where a page writes the pay's figures with the site's
  own font (private characters drawn as digits), ATP reads them back as
  digits;
- `city`, `experience`, `degree` and `remote` (work from home, 在家办公).

BOSS Zhipin's own: `district` and `business_district`, `days_per_week`
and `months` (of internships), `address` (job pages), `category` (the
kind of work, in `hot`), `company_industry`, `company_stage` and
`company_size`. Skills and benefits are tags. The site's cards carry no
dates.

## Limits

- The job search, job pages and company pages are read through a
  browser without a window, which the site's security check has turned
  away (see Risk control); `hot` is what has been seen working without
  an account.
- Nothing that needs a login has been tried with a real account.
- Sending messages is not offered: the site's chat sends over its own
  socket protocol. Neither are exchanging phone numbers or WeChat ids,
  sending résumés or greeting in bulk, nor anything of the recruiter's
  side.
- Recruiters (names, titles, pictures, account ids, whether they are
  online) and contact details are dropped from every payload, `raw`
  included.
- No filters by job function, district, metro line, part-time pay or
  schedule; no sort orders (the site has none).
- A Chromium-family browser is needed for the search, job pages and
  company pages, and for account requests the site answers with its
  environment check; without one they fail with `internal_error`.
