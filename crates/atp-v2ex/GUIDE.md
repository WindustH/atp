V2EX through its official API: API 1.0 needs no account; API 2.0 takes a
personal access token and adds your account, notifications, and paging
through long reply lists and node listings. A browser's login session
adds what only the site's pages have: the daily check-in and your coin
balance.

## Logging in

### Personal access token

1. Sign in on https://www.v2ex.com and open Settings → Tokens
   (https://www.v2ex.com/settings/tokens).
2. Create a token. V2EX offers lifetimes of 30, 60, 90 or 180 days and two
   scopes: `everything`, or `regular` (which cannot create further tokens);
   either works for ATP.
3. `atp v2ex login --token TOKEN` (`-` reads it from stdin). The token is
   checked against your account at once and kept in ATP's session file
   (readable by you only); `V2EX_TOKEN` supplies one for a single run
   instead and is never saved.

The token unlocks `whoami`, `notifications`, `delete-notification` and
`token`, and makes `read`, `comments`, `node` and `topics` use API 2.0
(`comments` and `topics` then go page by page).

### Browser session

The daily login reward and the coin balance are on the site's pages, not
in the API, so `check-in` and `balance` take the login session of a
browser instead of a token:

1. Sign in on https://www.v2ex.com in a browser.
2. `atp v2ex login --browser` (or plain `atp v2ex login`) imports its
   v2ex.com cookies; `--cookie '...'` takes the `Cookie` header of a
   www.v2ex.com request instead. The `A2` cookie (the login) is
   required. `ATP_V2EX_COOKIE` supplies such a header for one run,
   without saving it.

The session and the token are kept side by side: the session's cookies go
only to the site's pages (`check-in`, `balance`), and the API calls keep
using the token without them. `whoami` and `status` show the session's
account when there is one, else the token's; `login --token` checks the
token. `logout` removes both.

How long a session lasts is not known. When V2EX sends ATP to its sign-in
page, `check-in` and `balance` fail with `not_authenticated`; log in in
the browser and import the session again.

### Without an account

`hot`, `read`, `comments` (all replies at once), `user`, `user-posts`,
`node`, `topics` (the node's first page) and `nodes` use API 1.0. V2EX
has no search API, so there is no `search`.

### How long it lasts

A token works until the lifetime chosen when it was created runs out, or
until you delete it on the tokens page; `atp v2ex token` shows its scope
and lifetime. A token V2EX no longer accepts fails with
`not_authenticated`; create a new one and log in again.

### Limits and pacing

V2EX allows an IP 600 requests an hour across both APIs (the
`X-Rate-Limit-*` headers count down). ATP spaces requests at least 1.2
seconds apart and notes when fewer than 30 are left in the hour; beyond
the limit commands fail with `rate_limited`. Deleting a notification and
claiming the daily reward wait half a second to a second and a half
first. Requests (the site's pages too) name ATP as their client
(`User-Agent`): V2EX's Cloudflare stops clients that look like a browser
but run no JavaScript. Should it stop ATP too, commands fail with
`verification_required` at once: a check passed in a browser does not
reach ATP, so none is opened.

### Environment

`V2EX_TOKEN`: a token for one run. `ATP_V2EX_COOKIE`: a login session's
`Cookie` header for one run.

## References

- Topics: a link (`https://www.v2ex.com/t/1248653`, a `#reply…` part is
  ignored), `t/<id>`, or the id.
- Members: a profile link (`https://www.v2ex.com/member/Livid`),
  `member/<name>`, `@name`, or the name.
- Nodes: a link (`https://www.v2ex.com/go/python`), `go/<name>`, or the
  name.
- `#N` works after any list.

## Commands

### hot

`--category hot|latest`: today's hot topics (by default) or the latest
ones. API 1.0 hands out one short list for each.

### read

A topic: title, text (Markdown or plain, as written), node (in `tags` and
`extra.node`), reply count, last reply, and appended notes
(`extra.supplements`).

### comments

The topic's replies in floor order; each has its floor in `extra.floor`.
Without a token they come all at once; with one, page by page. Replies on
V2EX are flat, so there is no `replies`.

### user, user-posts

A member's profile (bio, tagline, website, GitHub, location, member
number, join date); `user-posts` lists their recent topics.

### notifications

Your notifications (needs a token), with the kind read from the sentence
(`reply`, `mention`, `like` for thanks, `favorite`, else `system`), the
topic link and the text it is about.

### node, topics, nodes

`node NAME` shows a node; `topics NAME` lists its topics (only the first
page without a token); `nodes` lists every node, the most topics first.

### delete-notification

`delete-notification ID [-y]` removes one of your notifications (needs a
token).

### token

The token in use (masked), its scope, lifetime and use.

### check-in

`check-in` (or `checkin`) claims today's login reward (每日登录奖励)
with the login session: it opens the reward page
(https://www.v2ex.com/mission/daily), follows its redeem link (with the
`once` token the page carries) and reads the page again to see the reward
gone. It answers with the bronze coins the button offered and the balance
after, or "already checked in today" without claiming anything. Pages are
read by their links and coin pictures, so the account's language does not
matter.

### balance

Your coins, from https://www.v2ex.com/balance: `gold`, `silver`,
`bronze`, all of them counted in bronze (`total_bronze`; 100 bronze make a
silver, 100 silver a gold), and the account (`user`). Needs the login
session.

## Limits

No search (V2EX has no search API), no posting, replying, thanking or
favoriting, no marking notifications read, and API 1.0 shows only a
member's recent topics. The coin history on the balance page is not read.
