1Point3Acres (一亩三分地, 1point3acres.com), the forum of Chinese students
and workers abroad: interview reports (面经), referrals (内推), offers,
visas, studies and life. ATP reads it through the JSON API the site's own
pages call: the hot and newest threads, the boards and their threads,
threads with their replies; with the site's cookies also the search,
profiles, a user's threads, every answer to a reply and replying to a
thread. `1p3a` and `yimu` are shorter names for the platform.

## Logging in

Most reads need no account. A login is the site's own cookies of
`.1point3acres.com` (Discuz's `…_auth` cookie among them), which the
site's pages send to its API; ATP sends them the same way.

- `atp 1point3acres login --browser` reads the cookies of a browser
  logged in at www.1point3acres.com. Log in there first (the site's
  login page is auth.1point3acres.com; the forum's pages may ask the
  browser to pass a Cloudflare check before they show).
- `atp 1point3acres login --cookie '…'` takes a `Cookie` header copied
  from the browser's developer tools (a request of a logged-in page to
  `api.1point3acres.com`; `-` reads it from stdin).
- `ATP_1POINT3ACRES_COOKIE='…'` passes the cookies for one run; they
  are never saved.

`login` and `status` ask the site who the cookies belong to (the
`user.me` call its pages make) and fail with `not_authenticated` when it
sees no login. Phone, password, WeChat and Google logins are not offered.
This login has not been tried with a real account.

### Without an account

`hot`, `feed`, `read`, `comments`, `boards` and `board`.

### With an account

`whoami`, `status`, `search`, `replies`, `user`, `user-posts` and
`comment`; the API answers 请先登录 (`not_authenticated`) to them without
one. A login may
also show parts of threads the site keeps from guests, as far as the
account's points (积分) allow.

### How long it lasts

How long the site keeps a login is not published. When it ends, commands
fail with `not_authenticated` and the hint to log in again.

### Risk control

- ATP keeps at least 3 s between requests and waits 2 to 5 s before a
  reply, and retries nothing. The site does not publish its limits;
  refusals for coming too often are `rate_limited`.
- Cloudflare guards the forum's pages (`/bbs/…`, `/home/…`) and, from
  some networks, answers with a challenge. ATP never reads those pages,
  only the API; should the API answer with a challenge, the command
  fails with `verification_required`, and ATP does not open it in a
  browser: a pass there would not carry over to ATP's requests.
- The site's check-in (签到) and daily question ask for a Turnstile
  captcha; ATP offers neither. A reply the site answers with a captcha
  fails with `verification_required`, unanswered.
- Some actions need the account bound to WeChat first (此操作需要先进行微信
  绑定); the site's refusal is `permission_denied`.

### Environment

`ATP_1POINT3ACRES_COOKIE`: the session's cookies for one run.

## References

- Threads: `https://www.1point3acres.com/bbs/thread-<tid>-1-1.html`
  (what lists print; any page of it), `/home/thread/<tid>`,
  `/home/pins/<tid>`, `forum.php?mod=viewthread&tid=<tid>`, a
  `goto=findpost` link (its `ptid`), or the number.
- Replies: the id `comments` prints, `/home/post/<pid>`, or a
  `goto=findpost` link (its `pid`).
- Users: `https://www.1point3acres.com/home/u/<uid>`,
  `space-uid-<uid>.html`, `home.php?mod=space&uid=<uid>`, or the number.
  Names (`space-username-…`) are not looked up.
- Boards: `forum-<fid>-1.html`, `/home/forum/<fid>`, the number, or the
  board's name as `boards` lists it (海外面经).
- `#N` works after any list.

## Commands

### hot

The hot threads (热门), as the site's discover page lists them, 20 a
page; the list is ranked, not dated.

### feed

The newest threads of the whole forum (最新主题), 20 a page. It needs no
login and is the same for everyone: the site's personal feeds are not
read.

### read

A thread with its first post as the text: title, text (the post's BBCode
as plain text: links as `text (link)`, quotes as `> ` lines, lists as
`- ` items, `[attachment]` where a file was attached), pictures, author,
board and section (`extra.board`), thread type (`extra.type`), topic tags
(companies, places, schools), views, replies, saves (`favorites`),
upvotes (`likes`), downvotes, heat and the site's rating figure (评分,
`metrics.rate`), the last reply (`extra.last_reply_at`,
`extra.last_poster`) and the marks `digest` (精华), `pinned`, `closed`
and `paid`. Anonymous posts (地里匿名用户…) have an author without a
number (`extra.anonymous`). Hidden parts: see Hidden content.

### comments

The replies of a thread, 20 a page, each with up to five answers to it
nested (`replies`, `reply_count` counting them all, `reply_to` naming
whom an answer answers). `--sort oldest` (the default, the thread's
order), `newest` or `hot`; only `oldest` has been tried. Folded replies
(the site folds downvoted ones) carry `extra.folded`. The answers past
the fifth need a login (`replies`, or `comments --replies`).

### replies

`replies THREAD REPLY` lists every answer to one reply, 20 a page; the
thread is not needed to find them. The API answers guests 请先登录, so it
needs a login. Not tried with a real account.

### search

`search WORDS` finds threads by words. It needs a login: the site's API
answers 请先登录 to guests. There is no sort or filter. Not tried with a
real account.

### user, user-posts

`user` shows a profile: name, avatar, the day the account was made,
followers, following, threads (`stats.posts`, all posts in
`stats.all_posts`) and points (`extra.credits`). `user-posts` lists the
threads a user started, 20 a page. Both need a login, as on the site, and
take the user's number or profile link. Not tried with a real account.

### comment

`comment THREAD TEXT` replies to a thread under your name (never
anonymously), as the site's pages send a reply; the text is sent as
written (Discuz BBCode works). `--reply-to` and pictures are not
supported. The result carries the new reply's id when the site returns
it. Not tried with a real account.

### boards

Every section (`kind: section`) followed by its boards and sub-boards
(`kind: board`), with their numbers, where each sits (`description`:
`海外求职 › 海外面经`), the parent board (`extra.parent`) and the thread
types the board sorts by (`extra.types`, number → name). The list is kept
a day. Boards the site has turned off are left out.

### board

`board BOARD` lists the threads of a board (not a section), 20 a page,
with their summaries where the site gives them. `--sort new` (the
default, newest threads first), `replied` (latest replies first, the
site's own default on most boards), `hot` or `digest` (only 精华
threads). `--type` keeps one thread type, by its name or number as
`boards` lists them (`--type 我司要招人` on 海外职位内推). Only `new` has
been tried; the other orders are those the board pages offer.

## Hidden content

Some threads keep their text from readers without enough points or
without a login: the site sends its own notice in their place
(`本帖内容需要积分高于 200 才可浏览`). ATP shows the text as sent, marks the
thread with `extra.hidden` and the points asked for
(`extra.hidden_credits`), and says on stderr that part of it is hidden.
It never tries to unlock such parts. Boards of 面经 show such threads
with no summary and no views to guests.

## Job posts

Threads offering a job are `kind: job`: those of 海外职位内推 (198) and
中国招聘信息 (322) with the type 我司要招人 or Hiring Manager 直招, and
every thread of ✨企业直聘 (266). They carry the job keys where the thread
shows them: `company` from its company tag, `city` from its place tag,
and the pay (`salary`, `salary_min`, `salary_max`, `salary_period`,
`salary_currency`) where the title names it (`$180k-220k`, `30-50K`).
Requests for a referral (求内推), 面经 and every other thread stay
`kind: thread`. The structured fields of 内推 and 面经 posts (position,
interview stage, outcome) are coded numbers ATP does not read.

## Limits

- No new threads, editing, deleting, upvotes, reactions, ratings (加米),
  polls or saving threads: their calls were not identified well enough,
  or need what ATP cannot answer.
- No notifications, private messages, favorites or reading history.
- No check-in (签到) and no daily question: both need a Turnstile captcha.
- No 面经 question bank, offer reports database, salaries or visa tracker
  (the site's other tools).
- Attachments are marked, not listed or downloaded.
