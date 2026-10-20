Pinterest, the site of pins: pictures and videos saved to boards. ATP
talks to the resource API the site's own pages call: the search of pins,
boards and people, pins with their original picture or video, profiles
with their pins and boards, and a board's pins. With the site's cookies
it also saves pins to your boards, creates pins from pictures on the web
and creates boards.

## Logging in

Searching and reading need no account where Pinterest shows them to
guests. A login is the site's own cookies of `.pinterest.com` (the
session `_pinterest_sess`, `_auth=1` once logged in, and `csrftoken`,
which actions send back as their token), sent as the site's pages send
them.

- `atp pinterest login --browser` reads the cookies of a browser logged
  in at www.pinterest.com. Log in there first (e-mail and password,
  Google, Facebook or Apple). A login on a country site such as
  pinterest.co.uk keeps its cookies there; log in at www.pinterest.com.
- `atp pinterest login --cookie '…'` takes a `Cookie` header copied from
  the browser's developer tools (any request of a logged-in page to
  www.pinterest.com; `-` reads it from stdin).
- `ATP_PINTEREST_COOKIE='…'` passes the cookies for one run; they are
  never saved.

`login` and `status` ask for the account's settings
(`UserSettingsResource`), which name it, then show its profile. ATP
offers no password login of its own. This login has not been tried with a
real account.

### Without an account

`search` (pins, `-t user` people, `-t topic` boards), `read`, `user`,
`user-posts`, `collections USER`, `board` and `download`. Pinterest is
closing more of its pages to guests (its "logged-out lockdown"), varying
by country and network; what it refuses to a guest fails with
`not_authenticated` and the hint to log in.

### With an account

`whoami`, `status`, `collections` (yours), `favorite`, `folder-create`
and `create-pin`.

### How long it lasts

How long the site keeps a login is not published. When it ends, `_auth`
is gone and the commands of an account fail with `not_authenticated` and
the hint to log in again.

### Risk control

- ATP keeps at least 3 s between requests and waits 1 to 3 s before
  each action. Reads are tried again twice on network errors; actions
  never. HTTP 429 is `rate_limited`.
- Downloads come from the image host (i.pinimg.com) and are not paced.

### Environment

`ATP_PINTEREST_COOKIE`: the session's cookies for one run.

## References

- Pins: `https://www.pinterest.com/pin/<id>/` (what lists print; any
  country site, `jp.pinterest.com`, `pinterest.co.uk` …), the short
  `https://pin.it/<code>` of the share button (followed to its pin), text
  pasted with such a link in it, or the number.
- Users: `https://www.pinterest.com/<username>/`, the username, or the
  username with `@`.
- Boards: `https://www.pinterest.com/<username>/<board>/`,
  `<username>/<board>` (what `collections` prints as `id`), or the
  board's number (`extra.board_id`).
- `#N` works after any list of pins, people or boards.

## Commands

### search

The site's search of pins, twenty-five a page, as its search box ranks
them; ads are left out. `-t user` searches people, `-t topic` boards.
There are no `--sort` or `--filter` values. A guest's results name their
pins by ids of their own (`e1_…`), which open only within the visit that
listed them: ATP keeps the pins of those lists for a day (the latest
hundred), so `read` and `download` of them answer from the list, with
what the list showed (the biggest picture it gave, not always the
original).

### read

A pin with its title, description, the picture in original size (or the
best MP4 of a video pin, or the pages of an idea pin), the account that
saved it (`author`), its creator when someone else saved it
(`extra.creator`), saves (`favorites`), comments, reactions (`likes`),
the link it leads to (`extra.link`, `extra.domain`), its board
(`extra.board`) and its main color.

### user

A profile: name, username (`handle`), picture, about, website, followers,
following (people and topics together), pins (`stats.posts`), boards,
monthly views (`stats.monthly_views`), the verified mark and when the
account joined.

### user-posts

The pins a profile lists, twenty-five a page.

### collections

A user's boards (yours without USER, which needs a login), with their
description, pins (`items`), followers, sections and owner; secret
boards show to their owner only (`extra.privacy`). Each board's `id` is
`<username>/<board>`, which `board` and `--folder` take.

### board

`board BOARD` lists the pins of a board, twenty-five a page.

### favorite, unfavorite

`favorite PIN --folder BOARD` saves a pin to one of your boards; without
`--folder`, to your profile. `unfavorite` is refused: a save is a pin of
your own, which ATP does not delete. Not tried with a real account.

### folder-create

Creates a board: public, or secret with `--private`; `--desc` gives its
description. Not tried with a real account.

### create-pin

`create-pin IMAGE_URL --board BOARD` creates a pin on one of your boards
from a picture on the web, which Pinterest fetches itself; `--title`,
`--description` and `--link` (the page it leads to) are optional. Local
files are not uploaded. Not tried with a real account.

### download

Saves a pin's picture in original size, its video (the best MP4; pins
offering only a streaming playlist have nothing to download) or the
pages of an idea pin.

## Limits

- No home feed, notifications, comments, likes, follows or messages.
- No uploads of local files, editing or deleting pins, boards or board
  sections.
- Pins of a guest's lists open only from the list ATP kept (see
  `search`); a numeric link opens any time.
- The logged-in parts (`collections` of your own, `favorite`,
  `folder-create`, `create-pin`, `whoami`) were written to the requests
  the site's pages send and have not been tried with a real account.
