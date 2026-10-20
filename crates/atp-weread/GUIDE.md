WeRead (微信读书) through the routes weread.qq.com's web reader calls: the
store's search, rankings and categories, and with a login a book's details
and public reviews (点评), your shelf, how far you are in each book, and
your own highlights (划线) and thoughts (想法), which `notes` lists in
reading order or writes as Markdown. With an API key of WeRead's official
Agent Gateway, these reads go through the gateway instead, and
recommendations (`feed`), similar books (`related`) and your reading
statistics (`reading`) join them. ATP never fetches the text of books:
they are paid, protected content. It reads tables of contents, WeRead's
AI outline of a book and the passages around a searched word. Your notes
are your own data.

## Logging in

### API key (Agent Gateway)

`atp weread login --token wrk-…` keeps a personal API key of WeRead's
official Agent Gateway (`https://i.weread.qq.com/api/agent/gateway`),
the route of WeRead's own agent skill (weread-skills). WeRead hands these
keys (`wrk-` and letters) to your account for that skill; where exactly
is up to WeRead (its skill page, https://weread.qq.com/r/weread-skills).
`WEREAD_API_KEY` passes a key for a run and wins over the saved one.
`login --token` checks the key with one gateway call; the gateway does not
name its reader, so `whoami` shows the account as `WeRead API key
wrk-…` (with the user number when a cookie session is there too).

With a key, ATP prefers the gateway for `search`, `read`, `comments`,
`favorites` (the shelf), `notes`, `progress` and `stats`, and only the
gateway serves `feed`, `related` and `reading`, and `search --filter` and
`comments --sort`. `user` and the cookie extras below still need a cookie
session; `login --token` leaves the session's cookies alone, so both can
be there.

ATP sends the skill version it was written against (`skill_version`
1.0.4) with every call. When the gateway answers with `upgrade_info`, ATP
prints a warning that this version should be bumped, and goes on with the
answer. A refused key (HTTP 401, as a made-up key gets, or `errcode`
-2010 / -2012) is `not_authenticated`: make a new key and log in with
it. How long a key lasts is not known. Everything the gateway answers to
a valid key is untested so far: it needs a key.

### QR code

`atp weread login` shows a QR code of weread.qq.com's login. Scan it with
WeChat and confirm. WeRead sometimes asks for the four-digit code WeChat
then shows; ATP asks for it on the terminal (without a terminal, use
`--browser`). The login keeps the cookies the web reader keeps: `wr_vid`
(your user number), `wr_skey` (a short-lived key), `wr_rt` (the token that
renews it) and `wr_fp` (a random id of this install, sent as the site
sends its browser fingerprint, so that logging in here does not end your
other WeRead web sessions).

### Browser or cookie

- `atp weread login --browser` reads the cookies of a browser logged in at
  weread.qq.com.
- `atp weread login --cookie '...'`: the `Cookie` header of a request to
  weread.qq.com. It must hold `wr_vid` and `wr_skey`, and should hold
  `wr_rt`, without which the session cannot renew itself (below).
- `ATP_WEREAD_COOKIE='...'` passes such a header for one run; it is never
  saved.

### Without an account

`search`, `hot` and `category` work without logging in, and so does
`book-search` (checked live). Everything else (`read`, `comments`,
`user`, `favorites`, `notes`, `progress`, `stats`, `chapters`) needs a
cookie session or a key: WeRead answers visitors `-2010 用户不存在`.
`outline` answers without an account, but needs the chapter list first,
which takes a session or a key.

### How long it lasts, and how its end shows

`wr_skey` is short-lived (the web reader renews it all the time; its
lifetime is not published). Once it has expired, WeRead answers `-2012
登录超时`; ATP renews the key with `wr_rt` through `/web/login/renewal`,
as the web reader does, keeps the new cookie and repeats the request, so
this is usually invisible. How long `wr_rt` itself lasts is not published; once it
has expired too, or the session was ended elsewhere, commands fail with
`not_authenticated` ("the session expired and could not be renewed",
`-2013 鉴权失败` or `-12013`) and the hint to log in again. A cookie
imported without `wr_rt` fails as soon as its key expires.

### Risk control

No captcha flow is known on these routes. ATP keeps at least 0.8 s
between requests and does not write anything to your account.

### Environment

- `WEREAD_API_KEY`: an Agent Gateway key (`wrk-…`); it wins over the
  saved one.
- `ATP_WEREAD_COOKIE`: a cookie header for one run, never saved.

## References

- Books: the book id (`695233`; official-account books `MP_WXS_…`), a
  reader link (`https://weread.qq.com/web/reader/<hash>`, also with a
  chapter), a detail link (`https://weread.qq.com/web/bookDetail/<hash>`),
  a share link (`https://weread.qq.com/book-detail?v=<hash>`), or `#N`.
  The hash is decoded to the id and checked.
- Users: the user number (`vid`, as `whoami` shows it).

## Commands

### search

The store's search (books, also web novels and audiobooks); each result
has its place, readers, rating and price. With a key, `--filter` picks
the gateway's scope: `all` (the default), `ebook`, `webnovel`, `audio`,
`author`, `fulltext` (inside books), `booklist`, `mp` (official accounts)
or `article`; results that are not books (authors, lists) are left out.

### hot

A ranking by `--category`: `rising` (飙升榜, the default), `newbook`
(新书榜), `novel` (小说榜), `all` (总榜), `newrating` (神作榜), `potential`
(神作潜力榜), `hot-search` (热搜榜). Each book carries its place
(`extra.rank`), readers (`metrics.readers`), the share of readers who
recommend it (`extra.recommend_percent`) and WeRead's verdict
(`extra.rating_title`, such as 神作).

### feed

Books WeRead recommends to you, each with its reason (`extra.reason`).
Gateway only: it needs a key.

### read

A book's title, author, translator, publisher, introduction, category,
ISBN, words, price, rating and publication date.

### comments

Public reviews (点评) of a book, in the order WeRead lists them, with the
reviewer's verdict (`extra.rating`: recommend, fair or poor) and stars
where given. With a key, `--sort` picks which: `all` (the default),
`recommend`, `poor`, `newest` or `fair`.

### user

A reader by number: name, avatar, signature. It needs a cookie session.

### favorites

Your shelf: books, audio albums (`kind` `album`) and official accounts
(`mp`), the most recently read first, each with its progress in percent
(`extra.progress`) and reading time (`extra.reading_seconds`) where the
shelf says.

### notes

`notes` lists the books you highlighted or wrote thoughts in, the latest
first, with their counts. `notes BOOK` lists your highlights (`kind`
`highlight`) and thoughts (`thought`, with the passage in `extra.quote`)
plus your review of the book (`review`), in reading order, each with its
chapter. `notes BOOK --markdown FILE` writes them as Markdown instead:
chapters as headings, highlights and passages as quotes, thoughts as
text. Through the gateway, `notes` pages by the gateway's cursor and
`notes BOOK` reads your thoughts in one call (`/review/list/mine`).

### progress

`progress BOOK`: the percent read, the chapter, the reading time in
seconds, when you started, finished and last read, and the text near
your position.

### stats

Totals of your shelf (books, albums, finished, started, reading time
where the shelf says) and of your notes.

### reading

`reading [--period week|month|year|all] [--at WHEN]`: your reading
statistics from the gateway (`/readdata/detail`): days read, reading
time and the daily average (seconds), the change from the period before,
the books read longest, counts, and your preferred categories, authors,
publishers and hours of the day. `--at` picks a time inside the period
wanted (`2025-06-01`, `30d`); the current period by default. Gateway
only: it needs a key.

### category

`category ID`: the books of a store category (`100000` 精品小说, `200000`
历史, `300000` 文学, `700000` 计算机, …; the ids appear in the store's
links) or of a ranking by its id.

### related

`related BOOK`: books like one, as WeRead suggests them. Gateway only: it
needs a key.

### chapters

`chapters BOOK`: the table of contents, one row per chapter with its
index, uid, title, level, words and price (yuan, for chapters sold one
by one). Through the gateway with a key, else through the web reader
(`/web/book/chapterInfos`, a cookie session).

### outline

`outline BOOK [--depth 2|3|4]`: WeRead's AI outline of the book
(`/web/book/outline`), one row per point with its chapter, number and
level (2 topics, 3 key points, 4 details), down to `--depth` (4). The
chapter list comes first (see `chapters`), so it needs a cookie session
or a key; the outline call itself was checked without one. Books WeRead
made no outline for are `not_found`. The whole command is untested so
far: it needs a login.

### book-search

`book-search BOOK QUERY`: the places a word or phrase occurs in a book,
in reading order, each a post of kind `match` with the passage around it
(about 150 characters) and its chapter (`extra.chapter_uid`, and
`extra.chapter_index` when WeRead gives it; `chapters` names them).
`-n` follows its pages (50 places each). It works without logging in.

## Limits

No book contents, no audiobook playback, no writing (no adding to the
shelf, highlights, thoughts or reviews), no friends' rankings. Reading
reports beyond `stats` and `reading` are not read.
