Pixiv (pixiv.net), the Japanese site of illustrations, manga, animated
illustrations (ugoira) and novels. ATP talks to the JSON API the site's
own pages call: artworks with every page in original size, novels with
their text, the search, the rankings, artists with their works and the
artists they follow, comments and downloads. With the site's cookies it
also lists your bookmarks and the new works of the artists you follow,
and likes, bookmarks and follows.

Works for adults (R-18 and R-18G) are never asked for or shown without a
login. With one, the site applies the account's own setting: an account
that hides such works does not get them through ATP either.

## Logging in

Reading works for all ages needs no account. A login is the site's own
cookies of `.pixiv.net`, sent as the site's pages send them; the one that
counts is `PHPSESSID`, which reads `<your user id>_<…>` once logged in
(the site gives guests one too, without the number).

- `atp pixiv login --browser` reads the cookies of a browser logged in at
  www.pixiv.net. Log in there first (accounts.pixiv.net: e-mail or pixiv
  ID and password, or Google, Apple, X).
- `atp pixiv login --cookie '…'` takes a `Cookie` header copied from the
  browser's developer tools (any request of a logged-in page to
  www.pixiv.net; `-` reads it from stdin).
- `ATP_PIXIV_COOKIE='…'` passes the cookies for one run; they are never
  saved.

`login` and `status` ask the site for the account's counters
(`/ajax/user/extra`), which it answers only to a session, then show the
profile with its number of followers and follows. ATP offers no password
login of its own. This login has not been tried with a real account.

### Without an account

`search`, `hot`, `read`, `comments`, `replies`, `user`, `user-posts`,
`novels` and `download`, for works for all ages: a guest's search asks
for those only, the rankings for adults are refused, and a work for
adults is refused before its pages or files are asked for.
`favorites USER` and `following USER` may answer too, as far as the site
shows them to guests.

### With an account

`whoami`, `status`, `feed`, `favorites` and `collections` (yours),
`following`, `like`, `favorite` / `unfavorite`, `follow` / `unfollow`,
`search --filter r18` and the rankings for adults (when the account's
settings allow such works).

### How long it lasts

How long the site keeps a login is not published. When it ends,
`PHPSESSID` stops naming the account and the commands of an account fail
with `not_authenticated` and the hint to log in again.

### Risk control

- ATP keeps at least 3 s between requests and waits 1 to 3 s before
  each action. Reads are tried again twice on network errors; actions
  never. HTTP 429 is `rate_limited`.
- The site sits behind Cloudflare. If its check answers instead of the
  site, the command fails with `verification_required`; ATP does not open
  it, since passing it in a browser does not clear ATP's requests. Try
  again later or from another network.
- Downloads come from the image host (i.pximg.net) with the site as
  `Referer`, which it requires; they are not paced.

### Environment

`ATP_PIXIV_COOKIE`: the session's cookies for one run.

## References

- Artworks (illustrations, manga, ugoira share their numbers):
  `https://www.pixiv.net/artworks/<id>` (what lists print; also with a
  language, `/en/artworks/<id>`), `https://www.pixiv.net/i/<id>`, the old
  `member_illust.php?illust_id=<id>`, an image link of i.pximg.net
  (`…/<id>_p0.png`), text pasted from the app's share button with such a
  link in it, or the number.
- Novels: `https://www.pixiv.net/novel/show.php?id=<id>` (what lists
  print) or `novel/<id>`. A bare number is always an artwork.
- Users: `https://www.pixiv.net/users/<id>` (also `/en/users/<id>`), the
  old `member.php?id=<id>`, or the number.
- Comments: the id `comments` prints.
- Bookmark tags (`--folder`): the tag, `private`, or `private:TAG`, as
  `collections` prints them.
- `#N` works after any list of works, users or bookmark tags.

## Commands

### search

The site's search by tag (any tag containing the words, as its search box
does by default), sixty works a page, newest first. `--filter` takes one
of: `illust` (illustrations and ugoira), `manga`, `ugoira`, `novel`
(novels instead of artworks), `safe` (artworks for all ages only) or
`r18` (artworks for adults only; needs a login whose settings allow
them). Without a filter it lists artworks of every kind; a guest gets
works for all ages only, a login what its settings allow. `--sort`:
`new` (the default), `old`, or `popular`, `popular-male`,
`popular-female`, which the site sorts by popularity for Premium
accounts only. Rows carry the work's thumbnail, tags, pages
(`extra.pages`), the AI mark (`extra.ai_generated`) and
`extra.restriction` (`R-18`, `R-18G`) when it applies; `read` gives the
rest. People are not searched (`-t user` is not offered).

### hot

The rankings, fifty works a page (five hundred for the daily one).
`--category` takes the site's ranking modes: `daily` (the default),
`weekly`, `monthly`, `rookie`, `original`, `daily_ai`, `male`, `female`,
and for adults `daily_r18`, `weekly_r18`, `male_r18`, `female_r18`,
`daily_r18_ai` and `r18g`, which need a login whose settings allow them.
Each work carries its rank (`extra.rank`), yesterday's
(`extra.previous_rank`), views, likes, pages and series. The rankings of
novels are not offered.

### feed

The new works of the artists you follow, newest first, page by page as
the site's page of them loads: `--type artworks` (the default) or
`novels`. Needs a login. Not tried
with a real account.

### read

An artwork with every page in original size (`media`, with their sizes),
its description as text, tags, artist (number, name, pixiv ID as
`handle`), views, likes, bookmarks (`favorites`) and comments, pages,
series (`extra.series`: id, title, order), the original and AI marks and
the rating. With a login, `extra.bookmarked` and `extra.liked` say what
the account did with it. A novel comes with its text: the site's markup
turned into plain text (`[newpage]` a rule, chapters a line of their
own, ruby as `漢字（かんじ）`, links as `text (url)`), its description
(`extra.description`), words, characters, reading time, language, cover
and pictures.

### comments, replies

The comments of an artwork or novel, newest first, fifty a page, with
their author, time (the site gives minutes in Japan time) and the user a
reply answers (`reply_to`); a stamp (a picture sent instead of words)
reads `[stamp <id>]`. The site tells only whether a comment has replies
(`extra.has_replies`), not how many, so `comments --replies` does not
fetch them; `replies WORK COMMENT` lists them.

### user

A profile as its page shows it: name, picture, self-introduction,
website and social links (`extra.website`, `extra.social`), how many it
follows, and the Premium and official marks. The region, age, birthday,
gender and job the site may hold are left out.

### user-posts, novels

`user-posts USER` lists a user's illustrations, manga and ugoira, newest
first; `novels USER` their novels. Both read the list of all their works
once, then the works themselves, up to 48 a page.

### following

The artists a user follows, as their page lists them (public follows
only), twenty-four a page, with their self-introduction and whether you
follow them. Without USER, yours (needs a login). Not tried with a real
account.

### collections, favorites

`favorites` lists bookmarked artworks, newest bookmark first: yours
without USER (needs a login), or a user's public ones. `--folder` narrows
them to a bookmark tag (`TAG`), takes your private bookmarks
(`private`) or the private ones with a tag (`private:TAG`).
`collections` lists the bookmark tags with how many bookmarks each has,
the private ones (yours only) with ids starting `private:`. Bookmarked
novels are not listed. Not tried with a real account.

### like

Likes an artwork (いいね!). The site cannot take a like back, so
`unlike` is refused. Novels are not liked here. Not tried with a real
account.

### favorite, unfavorite

`favorite ARTWORK` bookmarks an artwork publicly; `--folder private`
bookmarks it privately, and `--folder TAG` or `private:TAG` adds that
bookmark tag. `unfavorite` removes the bookmark (read from the work
first). Novels are not bookmarked here. Not tried with a real account.

### follow, unfollow

Follows an artist publicly, or stops following them. Not tried with a
real account.

### download

Saves an artwork's pages in original size, an ugoira as the site's zip
of its frames (the frames' timing is not saved), or a novel's cover and
pictures (its text comes with `read`). Works for adults are refused to
guests before anything is fetched.

## Limits

- Works for adults are never shown or fetched without a login.
- No search of people, no rankings of novels, no novel series and no
  related works.
- Bookmarked novels, private follows and the followers of an account are
  not listed.
- No comments, no new works, no editing; novels are not liked or
  bookmarked.
- The logged-in parts (`feed`, `favorites`, `collections`, `following`,
  `like`, `favorite`, `follow`, `whoami`) were written to the requests the
  site's pages send and have not been tried with a real account.
