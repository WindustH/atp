Liepin (猎聘, liepin.com), the job site for professionals, through the
requests its own pages make: the job search of the 职位 page with the
site's filters (city, pay, experience, degree, industry, company size and
funding, headhunter or company jobs), job pages with the whole
description, company pages and the jobs a company has open. With a
login: your saved jobs, your applications and their state, the
invitations to apply recruiters sent you, saving jobs, following
companies, applying to one job at a time and your chats with
recruiters.

## Logging in

### QR code

`atp liepin login` shows the QR code of liepin.com's login box. Scan it
with the Liepin app (猎聘 APP, "扫一扫") and confirm on the phone; the site
then sets the session's cookies, which ATP keeps. ATP polls every few
seconds, as the page does; when the site calls the code invalid or
expired, `login` fails and can be run again. This login has not been
tried with a real account.

### Browser or cookie

- `atp liepin login --browser` reads the cookies of a browser logged in
  at liepin.com.
- `atp liepin login --cookie '…'`: a `Cookie` header copied from the
  browser's developer tools (any request to `liepin.com`).
- `ATP_LIEPIN_COOKIE='…'` passes the cookies for one run; they are never
  saved.

The session needs the `UniqueKey` cookie (the account's id), which the
site sets on login. The chat commands also need the `imId_0` cookie (and
`send` the `imClientId_0` cookie) the site sets for its chat window; a
browser import after opening the site's chat brings them along. Texted
codes, passwords (behind the site's click captcha) and WeChat logins are
not offered.

### Without an account

`search`, `jobs`, `read`, `user`, `user-posts`, `cities` and
`industries` work without logging in.

### With an account

`whoami`, `favorites`, `favorite` / `unfavorite`, `follow` /
`unfollow`, `applied`, `invitations`, `apply`, `chats`, `messages`,
`send` and `unread`. None of them has been tried with a real account:
they follow the site's current pages, and the shapes of their answers are
read defensively.

### How long it lasts

How long the site keeps a login is not published. When it ends,
commands fail with `not_authenticated` (Liepin's codes `-1401` and
`5002`, or `-1701` when another login replaced it) and the hint to log
in again.

### Risk control

- ATP keeps at least 3 s between requests: the site's firewall answers
  bursts with HTTP 405, which ATP reports as `rate_limited`; wait a few
  minutes then. Saving, following, applying and sending wait 2 to 5 s
  more.
- The site sends a browser to a security check (安全验证) through a
  `TD-SecIntercept-Redirect` header. ATP reports it as
  `verification_required` and, on a terminal, opens the check in your
  browser with the session's cookies, then retries once; it never solves
  checks itself.
- The site limits how many applications an account sends a day; the
  numbers are not published. When the limit is reached, `apply` fails
  with the site's own message. Applying in bulk is what gets accounts
  restricted, so ATP applies to one job per command, after asking.

### Environment

`ATP_LIEPIN_COOKIE`: the session's cookies for one run.

## References

- Jobs: the job page links lists print, `https://www.liepin.com/job/<n>.shtml`
  (posted by a company) and `https://www.liepin.com/a/<n>.shtml` (posted
  by a headhunter); `m.liepin.com` links and the `/lptjob/<n>` links of
  company pages work too, as do `job/<n>`, `a/<n>` and the number alone
  (company jobs have ten digits beginning with 19, `1985552821`;
  headhunter jobs fewer, `78980551`).
- Companies: `https://www.liepin.com/company/<n>/` (or its
  `company-jobs` page), `company/<n>` or the number (`884492`).
- Chats: the id `chats` lists (the recruiter's chat id).
- `#N` works after any list.

## Commands

### search

`search WORDS` searches jobs all over China, 40 a page, in the site's
order (there is no other sort). `--filter headhunter` keeps the jobs
headhunters post for their clients (猎头职位), `--filter company` the
jobs companies post themselves (企业职位). `jobs` narrows further. Cards
carry no publication date, only the last refresh (`updated_at`), so
`--since` and `--until` keep every job.

### jobs

`jobs [WORDS]` is the search with the 职位 page's filters; without words
it lists every job the filters let through.

- `--city`: a city's name as the site writes it (北京, 杭州, 厦门 …), a
  province or district, or a code; the hot cities come from the search
  page, other names from the site's city suggestions (the closest match
  is taken; `cities NAME` shows them). All of China by default.
- `--salary 0-3k|3-5k|5-10k|10-20k|20-40k|40-60k|60k+`: monthly pay.
- `--experience graduate|intern|0-1|1-3|3-5|5-10|10+` (应届生, 实习生
  too).
- `--degree phd|master|bachelor|college|technical|high-school|middle`
  (博士, 硕士, 本科, 大专 … too).
- `--industry`: an industry's name (人工智能, 互联网 …), a group's name
  (AI/互联网/IT) or a code; `industries` lists them.
- `--size 1-49|50-99|100-499|500-999|1000-2000|2000-5000|5000-10000|10000+`.
- `--stage angel|a|b|c|d+|listed|strategic|undisclosed`: funding.
- `--kind headhunter|company`.

    atp liepin jobs rust --city 上海 -n 80
    atp liepin jobs 产品经理 --city 杭州 --salary 20-40k --experience 3-5

### cities, industries

`cities` lists the hot cities of the search page with their codes,
`cities NAME` the places matching a name (with their province).
`industries` lists the industries by group; a group's own entry covers
the whole group.

### read

A job's page: the whole description, the pay as shown and in the job
keys, city and address, experience, degree, industry, the openings
(招N人), whether students may apply, the benefits (as tags), the
recruiter's name and title as the page shows them, the company (with its
introduction on headhunter jobs) and the other requirements (语言要求 …).
Headhunter jobs often name the client only vaguely (某知名公司); the page
says what it says.

### user, user-posts

`user COMPANY` reads a company's page: name, logo, introduction,
industries, size, funding stage, benefits, the number of open jobs
(`stats.posts`), the address, and from the business register the
registered name, founding date, capital, type and state.
`user-posts COMPANY` lists its open jobs, 30 a page, as its 职位 tab does.

### favorites, favorite, unfavorite

Your saved jobs (收藏), 20 a page, one list without folders; `favorite
JOB` saves one and `unfavorite JOB` takes it out. Both read the job's
page first for the ids the site's buttons send.

### follow, unfollow

Follow a company (关注) or stop. The site shows no list of the companies
you follow, so there is no `following`.

### applied

Your applications (我的投递) with their state in `extra.status`
(`applied` 已投递, `viewed` 已查看, `interview` 面试邀约, `rejected` 暂不合适,
`expired` 反馈超时) and the site's words in `extra.status_label`, the
steps so far in `extra.history`, and `closed` when the job stopped
hiring. `--status applied|viewed|interview|rejected` keeps one state, as
the site's tabs do. Applications through companies' own career sites
(我的网申) are not listed.

### invitations

The invitations to apply (邀请应聘) recruiters sent you: the job, the
company, the pay, the recruiter's name and title, their note, and
`accepted` once you answered 感兴趣. Answering is `apply` on the job.

### apply

`apply JOB` applies to one job with your default online résumé, without
an attachment, the way the job page's 投递 does. It shows the job (title,
company, pay, city, link) and asks first; `-y` skips the question.
Before sending, the site may name something to do first, which ATP
reports instead of applying: finishing your résumé, a résumé its review
turned down, a company you block, or consent for a recruiter abroad.
There is one job per command, by design.

### chats, messages, send, unread

`chats` lists your chats with recruiters (name, title, company, unread
messages), 20 a page. `messages CHAT` shows a chat's messages, newest
first, 20 a page, without marking them read; messages trading phone
numbers or WeChat ids show as `[contact exchange]` with nothing of their
contents. `send CHAT TEXT` sends a text message to one of your chats;
it does not open new chats (which the site does with a greeting), and
attachments and résumés are not sent. `unread` counts the unread
messages of your 50 most recent chats.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in
`extra`, where the card or page tells them:

- `company`: the employer as shown (the `author` is the company, with
  its page when the site links one; headhunter jobs often hide it);
- `salary` as shown (`50-80k·24薪`, `250-300元/天`, `面议`), with
  `salary_min`, `salary_max` (per month), `salary_currency` (CNY) and
  `salary_months` (24 for `·24薪`) when the pay is monthly or yearly
  (yearly pay is divided by the months paid, 12 unless the text says);
  daily and hourly pay keep only `salary`;
- `city` (with the district, `北京-海淀区`), `experience`, `degree`, and
  `job_type: intern` for internships.

Liepin's own: `posted_by` (`headhunter` or `company`), `job_id` and
`job_kind` (the ids the site's actions take), `recruiter` (name and
title), `company_industry`, `company_size`, `company_stage`, `top`
(promoted), and on pages `address`, `industry`, `openings`, `students`,
`company_intro`, `other`. Cards date the job by its last refresh
(`updated_at`); pages also give the first publication (`created_at`).

## Limits

- No phone numbers or WeChat ids, ever: the site's contact exchange
  (交换电话 / 交换微信) is not offered, and contact fields are dropped from
  every payload, `raw` included.
- No opening new chats (聊一聊 sends a greeting in your name), no sending
  résumés or attachments in chats, no withdrawing applications.
- No `following`: the site has no list of followed companies.
- No feed or hot list: the personal recommendations of the home page
  are not offered.
- No editing your résumé or settings.
