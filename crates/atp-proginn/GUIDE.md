Proginn (程序员客栈, proginn.com), where developers find freelance work,
through the API of its job board (job.proginn.com, 兼职招聘): remote and
on-site (驻场) jobs with the board's filters (city, role and direction)
and job pages with the whole description. With the cookies of a
logged-in browser: the account, its unread counts and applying to one
job at a time. Developers' profiles on www.proginn.com are not read, and
whoever posted a job is left out of everything ATP prints.

## Logging in

### Browser or cookie

Log in on www.proginn.com in a browser, then:

- `atp proginn login --browser` reads that browser's cookies for
  `proginn.com`;
- `atp proginn login --cookie '…'` takes a `Cookie` header copied from
  the browser's developer tools (any request to `job.proginn.com` or
  `www.proginn.com`);
- `ATP_PROGINN_COOKIE='…'` passes the cookies for one run; they are never
  saved.

The session needs the `x_access_token` cookie, which the site sets on
`.proginn.com` when you log in. ATP offers no login of its own: the job
board's scripts only bind a phone number to an account that is already
logged in, and the login itself (texted codes, passwords) happens on
www.proginn.com.

### Without an account

`search`, `hot`, `jobs`, `read`, `cities` and `roles` work without
logging in.

### With an account

`whoami`, `status`, `unread` and `apply`. None of them has been tried
with a real account: they follow the board's current scripts, and the
shapes of their answers are read defensively.

### How long it lasts

How long a login lasts is not published. When it ends, the board answers
with status -99, which ATP reports as `not_authenticated` with the hint
to log in again.

### Risk control

- ATP keeps at least 2.5 s between requests; `apply` waits 2 to 5 s
  more.
- No captcha or other human check was met while reading the board, and
  ATP knows none to report; an unexpected answer is an `upstream_error`
  with the site's words.
- How many applications an account may send a day is not published.
  Applying in bulk is what platforms restrict accounts for, so ATP
  applies to one job per command, after asking.

### Environment

`ATP_PROGINN_COOKIE`: the session's cookies for one run.

## References

- Jobs: the link lists print, `https://job.proginn.com/d/<hash>`; the
  site's `/job/detail/<hash>` links (with `.html` or without) work too,
  as do the hash alone (`5af3107aa146`) and the job's number
  (`24902`, the `id` lists print).
- `#N` works after any list.

## Commands

### search

`search WORDS` is the board's own search, best matches first (not by
date), 20 jobs a page. `--filter remote` keeps remote jobs, `--filter
onsite` the on-site ones (驻场). Because the order is not by date,
`--since` and `--until` filter the jobs without ending the listing early.

### hot

`hot` lists every open job, newest first, as the board's front page
does; `--category remote` or `--category onsite` narrows it.

### jobs

`jobs [WORDS]` is the search with the board's filters; without words it
lists every open job, newest first.

- `--type remote|onsite`: remote work, or on-site work at the client's
  (驻场).
- `--city`: a city's name as the board writes it (北京, 苏州 …), the word
  of its links (`beijing`) or the id `cities` lists. The board takes the
  city along but, tried in October 2026, still listed jobs in other
  cities, so ATP keeps only the jobs in the city asked for. Remote jobs
  name no city and are left out; pages may come back with fewer jobs.
- `--role`: a role (后端, 前端, 设计师 …) or a direction within one (Rust,
  Python, UI设计师 …) as `roles` lists them, the word of the board's links
  (`rust`) or the id there (`1`, `1/112`). A name that is both a role and
  a direction (前端, 测试, 产品经理 …) is taken as the role.

### read

`read JOB` shows the whole description, the pay (`salary`, with
`salary_min` / `salary_max` per month in CNY), `remote` or the `city`,
the experience asked for, the `role` and `direction`, `status` (开放中
while the job takes applications) and `deadline`, the day the board
closes the job (a week after it was posted, in the jobs seen). Job pages
also count views and saves; lists count applications (`applications`).
The company is the post's author: the name it goes by, its full name,
logo and description. Nearly every job seen was posted by Proginn itself
(程序员客栈) for a client, whose name, if anywhere, is in the
description.

### cities, roles

`cities` lists the board's hot cities with their provinces, `cities
NAME` the cities whose name, link word or province matches (`cities
广东`). `roles` lists the roles (`role`) and the directions within each
(`direction`, id `ROLE/DIRECTION`). Both come from the board's own lists,
which ATP keeps for a day.

### unread

`unread` prints the counters of the site's message menu, under its own
names: `total`, `system` (系统消息), `work` (工作通知), `reply` (评论回复),
`at` (@我的), `community_other` (赞及其它) and `balance` (收支信息).

### apply

`apply JOB` applies to one job (申请接单) the way the button of a job page
does: it reads the job and the account first and stops, without sending
anything, when the job is no longer open, you applied already, the board
does not see the account as a developer, the account's real name is not
verified (实名认证), or the job is for developer members (开发者会员) and
the account is not one. Otherwise it shows the job and asks; `-y` skips
the question. The client then picks whom to hire among those who apply.

The site's pages show this button only inside the Proginn app (a browser
is sent to the app's download page). ATP sends the same request with the
browser session's cookies; whether the board accepts it from outside the
app is untested, and a refusal comes back with the site's words.

## Limits

- Developers' profiles and the developer lists of www.proginn.com are
  people's data and are not read.
- Who posted a job (`ownerInfo`, the poster's account id, when they were
  last online) and contact details (`emailShow`) are dropped from the
  output and from `raw`.
- Chatting with a client (沟通) happens in the Proginn app only. Saving
  jobs, the list of your applications, posting and managing jobs, and
  company pages are not offered.
- Everything that needs a login is untested.
