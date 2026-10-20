SMZDM (什么值得买, smzdm.com), where people share deals (好价) and write
about what they bought. ATP reads it without an account: the front
page's picks, the search, deals with their price, mall and the 值 / 不值
votes, articles, and the comments of both. `什么值得买` is a shorter name
for the platform. Nothing is bought, voted, saved, claimed or checked in.

## Logging in

There is nothing to log in to: every command works without an account,
`atp smzdm status` always reports "not logged in" and `whoami` is
`unsupported_operation`. The site's own logins (votes, saves, comments,
check-in) are not offered.

### The browser

The search site (search.smzdm.com) answers plain requests with a script
check that only a browser passes, so `search` reads through a browser
tab; everything else is read over plain HTTP.

- Chrome, Chromium, Edge or Brave must be installed; `ATP_BROWSER`
  names one by its path or command.
- It runs without a window on a profile of SMZDM's own,
  `<cache>/all-the-platforms/smzdm/browser`, kept between runs so the
  site's cookies last; `atp smzdm logout` deletes it.
- A page of the search site makes the search's requests once the site's
  script has passed. The browser is used as it is: no other user agent,
  nothing hidden or added to the site's pages.

### Risk control

- ATP keeps at least 3 s between requests and retries nothing; a refusal
  for asking too often is `rate_limited`.
- The search site may answer the browser with Tencent's captcha instead
  of passing it (it did in the one live try, without a window). The
  search then fails with `verification_required`; on a terminal ATP
  opens the page in a window of its browser, on the same profile, for
  you to pass, then searches again. ATP never answers a captcha.
- Should the deal and article pages, read over plain HTTP, ever answer
  with the check or the captcha, the command fails with
  `verification_required` and the page, which ATP does not open: a pass
  there would not carry over to those requests.

## References

- Deals (好价): `https://www.smzdm.com/p/<number>/` (what lists print),
  the mobile link (`https://m.smzdm.com/p/<number>/`), or the number.
- Articles (原创, 资讯): `https://post.smzdm.com/p/<id>/`, or the id
  (`ae6qdexk`). Notes (笔记): `https://post.smzdm.com/talk/p/<id>/`.
- Text pasted from the app's share button works when a link is in it.
- `#N` works after any list.

## Commands

### hot

The front page's picks (值得买精选, in the site's own mixed order), twenty
a page, as its list loads them: deals and articles with their price
(`price`: the figure, its currency and the site's wording in `label`),
mall (`extra.mall`), brand, channel (国内, 海淘 …), category
(`extra.category`), who shared it, 值 votes (`likes`), 不值 votes
(`metrics.unworthy`), saves, comments and the site's tags. `extra.sold_out`
and `extra.expired` mark deals that are over. Lists date a deal by its
latest update (`updated_at`, 更新时间) and an article by when it was
published (`created_at`); the site writes these times short (`16:41` is
today's, Beijing time; a bare `10-18` is a day).

### search

`search WORDS` lists the search's results across the site (综合): deals
and articles with their price, mall, time, 值 / 不值 votes, saves and
comments, page by page. It reads through the browser tab (see Logging
in). There is no sort or filter. Results have not been seen live: the
one try met the site's captcha (see Risk control). The rows are read
with the markup of the front page's list (`li.feed-row-wide`), untried
on the search's own pages.

### read

A deal: title, the price with its wording and the crossed-out price
(`price.original`), mall, who shared it (爆料人), the text (爆料原文,
小编补充), pictures (the deal's and its order screenshots), category,
labels (`tags`), 值 votes (`likes`), 不值 votes (`metrics.unworthy`),
saves, comments, and when it was posted and updated. Buying links are
left out.

An article or note: title, author, the body as text with its pictures,
topics (`tags`), likes, saves, comments, and when it was published and
updated.

### comments

The comments of a deal or article page by page as its pages show them,
each with the replies the page shows under it (`replies`; `reply_to`
names whom a reply answers), likes, the place it was posted from
(`location`) and `extra.author_of_post` on the poster's own comments.
`--sort newest` (the default, the page's 最新 tab) or `hot` (its 精彩
tab). Replies the page folds away are not fetched.

## Limits

- Read-only: no login, votes (值 / 不值), likes, saves, comments,
  sharing deals, coupons, check-in or buying.
- No users' pages, rankings, wiki (百科) or tests (众测).
- The search's sort orders and filters are not offered.
