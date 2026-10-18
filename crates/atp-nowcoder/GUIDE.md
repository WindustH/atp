Nowcoder (牛客网, nowcoder.com), where Chinese students look for campus
jobs and internships and trade interview write-ups, through the requests
its own pages make: the job search of its 校招, 实习 and 社招 pages with
their filters, job pages, the 校招 schedule (when companies take
applications, with referral codes and the companies' own application
pages), 面经, community posts and their comments, the hot lists. With a
login: saving jobs, your applications, the home page's 内推 tab,
subscribing to schedule entries, follows, likes, comments and unread
counts.

## Logging in

### WeChat QR code

`atp nowcoder login` shows the QR code of the site's WeChat login: the
code of Nowcoder's WeChat official account (牛客). Scan it with WeChat;
following the account signs in, and ATP keeps the login the site then
hands out as the `t` cookie. The code lasts the two minutes the site
gives it; ATP asks every few seconds whether it was scanned and fails
when it has expired. A WeChat account without a Nowcoder account is led
to the site's sign-up, which ATP does not do. This login has not been
tried with a real account.

### Token, browser or cookie

The site keeps its login in the `t` cookie of nowcoder.com.

- `atp nowcoder login --browser` reads the cookies of a browser logged in
  at nowcoder.com.
- `atp nowcoder login --token 'TOKEN'` takes the value of that cookie
  (developer tools, Application, Cookies, `https://www.nowcoder.com`,
  `t`; `t=TOKEN` works too, `-` reads it from stdin).
- `atp nowcoder login --cookie 't=…'`: a `Cookie` header holding it.
- `ATP_NOWCODER_COOKIE='t=…'` passes it for one run; it is never saved.

Phone, password and texted-code logins (behind the site's Geetest, Yidun
and Tencent captchas) and the Nowcoder app's own QR scan are not offered.

### Without an account

`search`, `jobs`, `careers`, `read`, `comments`, `replies`, `hot`,
`schedule` and `experience` work without logging in.

### With an account

`whoami`, `favorites`, `favorite` / `unfavorite` (jobs), `applied`,
`referral`, `schedule --tab subscribed`, `schedule-subscribe` /
`schedule-unsubscribe`, `like` / `unlike`, `follow` / `unfollow`,
`comment` and `unread`. None of them has been tried with a real account:
they follow the site's current pages, and the shapes of their answers are
read defensively.

### How long it lasts

How long the site keeps a login is not published. When it ends, commands
fail with `not_authenticated` (the site's code 999, `need login` or
`用户未登录`) and the hint to log in again.

### Risk control

- ATP keeps at least 1 s between requests and waits 2 to 5 s before each
  save, subscription, follow, like or comment. The site does not publish
  its limits.
- The site may want a captcha before taking a comment (its code 1125).
  ATP reports it as `verification_required` with the post's page and does
  not open it: a captcha passed in the browser does not carry over to
  ATP's request. Comment from the site then.
- The captchas of the site's logins are not met: ATP's logins are the
  WeChat QR code and a session imported from a browser.

### Environment

`ATP_NOWCODER_COOKIE`: the session's cookies for one run.

## References

- Jobs: `https://www.nowcoder.com/jobs/detail/<id>` (what lists print),
  `jobs/detail/<id>`, or the number (`467145`).
- Posts: moments (动态) `https://www.nowcoder.com/feed/main/detail/<uuid>`
  or the 32 hex digits of the uuid; long posts (帖子)
  `https://www.nowcoder.com/discuss/<id>`, or their number when it has 15
  digits or more (a shorter number is a job).
- Users: `https://www.nowcoder.com/users/<id>`, `users/<id>`, or the
  number.
- Companies of the schedule: `https://www.nowcoder.com/jobs/school/schedule/<id>`
  (what `schedule` prints), `https://www.nowcoder.com/enterprise/<id>`,
  or the number.
- Comments: their id, as `comments` lists it.
- `#N` works after any list.

## Commands

### search

`search WORDS` searches jobs, 20 a page: 校招 (campus hiring of new
graduates) by default, `--filter intern` internships, `--filter social`
experienced hires, all in the site's recommended order or with `--sort
new` the newest first. `--filter posts` searches the community's posts
instead (moments and long posts), by relevance, `--sort new` or `--sort
hot`. `jobs` narrows jobs further.

### jobs

`jobs [WORDS]` is the search of the 校招, 实习 and 社招 pages with their
filters; without words it lists every job the filters let through.

- `--kind campus|intern|social` (校招, 实习, 社招 too): campus by default.
- `--city`: a city as the site writes it (北京, 上海, 杭州 …); several give
  jobs in any of them.
- `--career`: a career category of the narrowest level (Java, golang,
  前端, 算法工程师 …) or its id; a broader one (后端开发) fails with the
  categories under it. `careers` lists them all.
- `--year 2027` (or `2027届`): graduation year; several give any of them.
- `--salary 0-3k|3-5k|5-10k|10-15k|15-20k|20-30k|30-50k|50k+`: monthly
  pay, for campus and experienced jobs (internships are paid by the day).
- `--nature state|central|bank|institute|foreign|private|public` (国企,
  央企, 银行, 研究所, 外企, 民企, 事业单位 too): who owns the company.
- `--size 0-20|20-99|100-499|500-999|1000-9999|10000+`: people; several
  give any of them.
- `--new`: the newest first instead of the site's recommendation.

The pages also list jobs of companies' own career sites (官网, applied to
through the site's 网申助手); they have no page on the site and are left
out, so a page may hold fewer than 20 jobs.

    atp nowcoder jobs 后端 --city 上海 --year 2027 -n 60
    atp nowcoder jobs --kind intern --career Java --city 北京 --city 杭州

### careers

`careers [NAME]` lists the career categories (三级职位类别) with their
ids, level and the groups above them (`软件开发 / 后端开发 / Java`);
`NAME` keeps those whose path contains it.

### read

A job: the whole description (duties and requirements), the pay, cities,
address, degree, graduation year, the deadline, the career category, the
keywords (as tags), how fast the company handles applications, and the
company with its industry and size. Internships add the days a week, the
months and whether they may turn into a job (`return_offer`). The
recruiter is not shown, not even with `--raw`.

A post: its title, its whole text (long posts' HTML as text), pictures,
topics (as tags), views, likes, comments, shares and saves, and where it
was posted from (`extra.location`, the province the site shows).

### comments, replies

The comments of a post, 20 a page, `--sort hot` (the default, as the page
opens) or `new`. Each says how many replies it has; `--replies` or
`replies POST COMMENT` fetches them. Commenters' schools, majors,
identities and IP addresses are dropped, `raw` included; the province the
site shows stays (`location`). Jobs have no comments.

### hot

`--category posts` (the default): the hot posts (热门帖子); `searches`:
the hot searches (热搜), each linked to its search; `topics`: the hot
topics (话题) with their views and posts; `recommend`: the home page's
recommended feed, page by page. The heat is in `metrics.heat`.

### schedule

`schedule [WORDS]` lists the 校招 schedule (校招日程), 20 entries a page:
a company's hiring for one batch (27届秋招, 27届实习, 日常实习 …) with
when applications open and close (`extra.opens`, `extra.deadline`, as
dates), the cities and careers, the referral code (`extra.referral_code`,
also in the title), the company's own application page
(`extra.apply_url`) and where the news came from (`extra.source`); the
text lists them for reading.

- `--tab recommended|closing|updated|supplementary|special|subscribed`:
  the page's tabs (推荐, 即将截止, 最近更新, 补招, 专场, 我的收藏);
  `subscribed` needs a login and takes no other filter.
- `--city` (several), `--industry` (人工智能, 银行 … several),
  `--type` (国企, 外企, 民企, 事业单位), `--batch` (27届秋招 …): names as
  the site shows them, or their ids; a part of a name finds the first
  that contains it.
- `--career`: a category of the middle level (后端开发, 产品经理 …); a
  narrower one stands for its group (Java for 后端开发).

The id of an entry is its company's, which `schedule-subscribe` takes.

    atp nowcoder schedule --tab closing --batch 27届秋招 -n 40
    atp nowcoder schedule 银行 --city 上海 --type 国企

### schedule-subscribe, schedule-unsubscribe

Subscribe to a company's schedule entry (收藏) or stop; subscribed
companies make up `schedule --tab subscribed`. Needs a login.

### experience

`experience` lists interview write-ups (面经) as the 面经 page does, 20 a
page: `--career` (any level; the site's first, 软件开发, by default),
`--company` (several; a name as the site writes it, looked up in its
suggestions, or the company's id) and `--new` for the newest first
instead of the site's order (综合).

### referral

The 内推 tab of the home page: posts offering referrals. Needs a login.

### favorites, favorite, unfavorite

Your saved jobs (收藏), and saving a job or taking it out; one list
without folders. Posts cannot be saved through ATP.

### applied

Your applications (投递记录) with the job, the company, the stage they are
at (`extra.status_label`) and the steps so far (`extra.history`, newest
first). `--stage viewed|passed|rejected` keeps one tab of the site's
(简历被查看, 通过初筛, 不合适). Needs a login.

### like, unlike

Like a post or take the like back. Jobs cannot be liked.

### follow, unfollow

Follow a user or stop.

### comment

`comment POST TEXT` comments on a post; `--reply-to COMMENT` answers one
of its comments. The comment answered is looked up among the first five
pages of the post's comments, for its author, which the site's form
takes; replies to replies are not supported. Text only.

### unread

Unread messages per kind: `system`, `likes`, `comments`, `follows`,
`messages`, `jobs`, and `total`.

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in `extra`:

- `company` (the `author` is the company, with its page);
- `salary` as the site shows it, with `salary_min`, `salary_max` per
  `salary_period`, `salary_currency` (CNY) and `salary_months`: campus and
  experienced jobs are paid by the month (`15-25K·14薪`), internships by
  the day (`300-350元/天`), some jobs by the year (`20-40万/年`);
  `薪资面议` (and the 0-999K some companies enter for it) has no figures;
- `city` (several joined with `/`), `degree`, `experience` (experienced
  hires), `job_type` (`full-time`, or `intern`) and `deadline` (the last
  day applications are taken, left out when the job takes them without
  an end).

Nowcoder's own: `recruit_type` (`campus`, `intern`, `social`),
`graduation` (`2027届`), `career`, `address`, `apply_url` (for jobs
applied to on another site), `days_per_week`, `months` and
`return_offer` (internships), `processing_rate` (percent of applications
handled) and `processing_days`, `company_industry` and `company_size`.

## Limits

- No applying to jobs: the site's application takes a résumé of the site
  and fills in company forms, and its one-step application sends several
  jobs at once; ATP does neither. The 网申助手 (one-delivery), which logs
  into companies' application systems with your passwords there, is not
  offered.
- No 题库, papers or practice (the paid question bank), no AI interview.
- No list of notifications: the site's message center is a page of its
  own, not checked; `unread` counts them.
- No posting, editing or deleting posts or comments, no saving posts, no
  private messages, no user profiles.
- Recruiters (their names, e-mail addresses, identities) and what the
  site tells about commenters beyond their name, level and province are
  dropped from every payload, `raw` included.
