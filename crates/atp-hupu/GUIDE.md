Hupu (虎扑, bbs.hupu.com), the forum of sports fans and much else:
步行街, NBA, football, games, gear, digital and cars. ATP reads the
forum's pages as a browser loads them, with the data they were rendered
from: the hot threads of the front page and of each zone, the boards and
their threads, threads with their replies and their 亮了 highlights. With
the site's cookies it also replies, lights replies (亮, Hupu's like) and
lists the mentions.

## Logging in

Reading needs no account. A login is the site's own cookies of
`.hupu.com` (the `u` cookie among them), sent along as the site's pages
send them.

- `atp hupu login --browser` reads the cookies of a browser logged in at
  bbs.hupu.com. Log in there first (passport.hupu.com: phone and code,
  password, or the app's QR code).
- `atp hupu login --cookie '…'` takes a `Cookie` header copied from the
  browser's developer tools (any request of a logged-in page to
  bbs.hupu.com; `-` reads it from stdin).
- `ATP_HUPU_COOKIE='…'` passes the cookies for one run; they are never
  saved.

`login` and `status` ask the front page whether it sees the session
logged in (`isLogin`) and fail with `not_authenticated` when it does not.
The account is shown by its number, the first field of the `u` cookie;
its name is not read. ATP offers no QR or password login of its own. This
login has not been tried with a real account.

### Without an account

`hot`, `read`, `comments`, `boards` and `board`.

### With an account

`whoami`, `status`, `comment`, `like-comment` and `notifications`.

### How long it lasts

How long the site keeps a login is not published. When it ends, the
actions fail with `not_authenticated` and the hint to log in again.

### Risk control

- ATP keeps at least 3 s between requests, waits 2 to 5 s before each
  action, and retries nothing. Refusals for acting too often are
  `rate_limited`.
- The site's pages sit behind Aliyun's WAF, which may answer with its
  check instead of the page. The command then fails with
  `verification_required` and the page; on a terminal ATP opens it in
  your browser with the session's cookies and tries once more. A check
  the site puts in front of an action (a slider) is not opened: ATP
  never answers it.
- Muted or banned accounts are refused by the site (`permission_denied`).

### Environment

`ATP_HUPU_COOKIE`: the session's cookies for one run.

## References

- Threads: `https://bbs.hupu.com/<tid>.html` (what lists print; any page
  of it, `<tid>-2.html`), the mobile and share links
  (`https://m.hupu.com/bbs/<tid>.html`), text pasted from the app's share
  button with such a link in it, or the number.
- Replies: the id `comments` prints.
- Boards: their link (`https://bbs.hupu.com/history`, or any list of it
  such as `history-postdate-2`), the part after the host (`history`,
  `topic-daily`, `502`), or the board's name as `boards` lists it
  (历史区).
- `#N` works after any list of threads or boards.

## Commands

### hot

The hot threads of the front page (about seventy, one page), with their
board, 亮 count (`metrics.lights`) and replies. `--category` takes a
zone's hot threads instead, by the name of its page: `gambia` (步行街),
`nba`, `soccer` (国际足球), `csl` (中国足球), `cba`, `sports` (综合体育),
`lol` (英雄联盟), `gg` (游戏), `ent` (影视娱乐), `gear` (装备), `digital`
(数码) or `cars` (汽车). The lists are ranked, not dated: their items
carry no time.

### read

A thread with its text (the post's HTML as plain text), pictures and
video, author (number, name, level, profile link), board (`extra.board`:
its place on the site, name, zone and numbers), views, replies, 推荐
(`likes`), 亮 count (`metrics.lights`), the time of the last reply
(`extra.last_reply_at`), the place the author posted from as the site
shows it (`extra.location`) and the app it was posted with
(`extra.client`).

### comments

The replies of a thread page by page as its pages show them, twenty a
page, oldest first (`--sort oldest`, the default), each with its 亮
count (`likes`), the number of replies to it (`reply_count`), the place
it was posted from (`location`), the reply it quotes (`reply_to` names
its author, `extra.quote` holds its id, author and text) and `extra.op`
on the thread starter's replies. `--sort lights` lists the thread's 亮了
highlights instead: the replies most lit, up to fifty, in one page. The
replies to a reply (楼中楼) are not listed: `replies` is not offered.

### comment

`comment THREAD TEXT` replies to a thread as the site's editor sends a
reply (each line a paragraph). `--reply-to REPLY` quotes one of its
replies, by the id `comments` prints. Pictures are not supported. Not
tried with a real account.

### like-comment

`like-comment THREAD REPLY` (also `light`) lights a reply: 亮, Hupu's
like, which the 亮了 highlights rank by; `--undo` takes the light back.
It reads the thread's board first, as the request names it. A reply lit
before answers 你已经点亮过这个回帖了, which ATP shows as done with that
message. Threads themselves are not liked (推荐 is not offered). Not
tried with a real account.

### notifications

The replies that mention the account (提到我的), newest first, each with
its text (the reply it answers quoted under it, `> …`), who wrote it,
the thread's title and a link to the reply. Other kinds of messages
(replies to you, lights, system messages) are not listed; `--type` is not
offered. Not tried with a real account.

### boards

The boards (版块) of every zone, as the front page lists them: their
place on the site (`id`), name, description, zone (`extra.zone`), number
(`extra.topic_id`) and the figure the site shows beside each
(`extra.count`). The front page may list only part of a big zone's
boards; any board's link works with `board`. The list is kept a day.

### board

`board BOARD` lists a board's threads, fifty a page: `--sort replied`
(the default, latest replies first, as the board opens), `new` (最新发布)
or `hot` (24小时榜). Each thread carries its author, time, views, replies
and 亮 count.

## Limits

- No search: the site's search page is under a path its robots.txt
  closes to robots, and ATP keeps out of it.
- No profiles and no user's threads: the site shows them to logged-in
  readers only, and their data was not identified.
- No new threads, editing, deleting, 推荐, saving threads, following or
  private messages.
- The replies to a reply (楼中楼) are not listed.
