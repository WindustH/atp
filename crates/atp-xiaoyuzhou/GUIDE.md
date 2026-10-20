Xiaoyuzhou (小宇宙) through the public pages of www.xiaoyuzhoufm.com:
shows with their newest episodes, episodes with their show notes, play
and comment counts, the hot comments an episode page shows, users with
the shows they host, and the episode's audio file, which `download`
saves. The pages carry this data for any visitor; ATP reads it from them
as a browser opening the page does. Logged in, ATP also reads the app's
API (api.xiaoyuzhoufm.com) for what only an account sees there:
transcripts (文字稿), the play history with how far each episode was
played, and every episode of a show.

## Logging in

### Without an account

Everything the public pages show works without logging in: `read`,
`comments`, `user`, `user-posts` (a show's 15 newest episodes) and
`download` (free episodes). `transcript` and `history` need a login.

### Texted code

`atp xiaoyuzhou login-sms [PHONE]` asks Xiaoyuzhou to text a code to the
number (mainland numbers as they are, others as `+CC NUMBER`; asked when
left out), then asks for the code on the terminal, as the app's login
does (`/v1/auth/sendCode`, then `/v1/auth/loginOrSignUpWithSMS`). A
number without an account gets a new one, as in the app. It needs a
terminal; ATP never reads texts itself. What happens when Xiaoyuzhou
wants a captcha first is not known: the login then fails with
Xiaoyuzhou's message. This login is untested so far.

### Tokens

`atp xiaoyuzhou login --token 'ACCESS_TOKEN|REFRESH_TOKEN'` (`-` reads it
from stdin) takes the app's token pair, the values of its
`x-jike-access-token` and `x-jike-refresh-token` headers, as copied from
the app's traffic. A refresh token alone is traded at once for a fresh
pair, which may log the app out, since each refresh replaces the refresh
token. Give both to leave the app logged in until the access token runs
out. Untested so far, as is the renewal below.

### What ATP sends

Calls to the app's API carry the iOS app's ordinary identification
headers (`BundleID: app.podcast.cosmos`, `App-Version: 2.98.0`, `OS`,
`Model` …; no secret key), a random device id made for this install and
kept with the session (`x-jike-device-id`), and the access token. ATP
keeps the tokens and the device id in its session file (readable by you
only).

### How long it lasts

The access token is short-lived; how long it lives is not published.
When the app's API answers 401, ATP trades the refresh token for a new
pair (`POST /app_auth_tokens.refresh`), saves it at once and repeats the
call, as the app does. How long a refresh token lasts is not known. Once
it is refused (or the account logged out elsewhere), commands fail with
`not_authenticated` and the hint to log in again. `atp xiaoyuzhou status`
shows the account (from `/v1/profile/get`; when that answer names no
account, the session is checked against the play history and the account
goes unnamed).

### Risk control

- ATP keeps at least 1.5 s between requests and writes nothing to the
  account.
- The most used open-source tool for the app's API warns in its README
  that logging in through it has got accounts flagged or banned. ATP
  sends what the app sends, but whether Xiaoyuzhou tells the difference
  is not known; use an account you can afford to lose, and log in
  rarely.
- If Xiaoyuzhou's firewall answers a public page with a check page
  instead (too many requests from one address), commands fail with
  `verification_required` and the page's address; open it in a browser,
  pass the check, and retry later or with a larger `--interval`. How
  often that happens is not known.
- The app's API answers 429 with `rate_limited`; wait before retrying.

### Environment

There is no environment variable and no cookie.

## References

- Shows: `https://www.xiaoyuzhoufm.com/podcast/<id>` or `podcast:<id>`.
- Episodes: `https://www.xiaoyuzhoufm.com/episode/<id>` (query
  parameters a shared link carries are ignored) or `episode:<id>`.
- Users (hosts and commenters): `https://www.xiaoyuzhoufm.com/user/<id>`
  or `user:<id>`.
- A bare id (24 hex digits): `read` tries it as a show, then as an
  episode; `user-posts` as a show, then as a user; `comments`,
  `download` and `transcript` take it as an episode.
- Links on other hosts (link shorteners) are followed to the page they
  lead to.
- `#N` works after any list.

## Commands

### read

A show: title, description, author, subscribers
(`metrics.subscribers`), episode count, image, latest release and its
hosts in `extra.podcasters`. An episode: title, the whole show notes as
text (timestamps included), release time, plays (`views`), claps
(`likes`), comments and favorites, the show as `author`, and the audio
file in `media` with its duration and size.

### user-posts

`user-posts SHOW` lists the show's episodes, the newest first. Without a
login these are the 15 the show's page lists (the website lists no older
ones); logged in, every episode, page by page, from the app's API
(`/v1/episode/list`; untested so far). `user-posts USER` lists the shows
the user hosts.

### comments

`comments EPISODE`: the hot comments the episode page shows (about 20),
each with the replies the page shows under it (`replies`) and the
commenter's IP region (`location`). There is no paging beyond them and no
newest-first order.

### user

A user's page: name, avatar, bio, IP region, followers, following,
subscriptions (`stats.subscriptions`), listening time
(`extra.played_seconds`) and the shows they host (`extra.shows`).

### download

`download EPISODE [-o DIR]` saves the episode's audio file, the file the
publisher uploaded or Xiaoyuzhou took from the show's feed, as
`<title> [<id>].<ext>`. A paid episode's page gives no audio file; logged
in, ATP asks the app's API for it (`/v1/episode/get`), which may give it
for episodes the account bought (untested so far). Otherwise it fails
with `permission_denied`.

### transcript

`transcript EPISODE` (also `subtitle`) prints the episode's transcript
as timed lines, the same shape as YouTube's `transcript` and Bilibili's
`subtitle`: `-f jsonl` / `-f csv` give one row per line (`from`, `to` in
seconds, `text`). It needs a login. ATP reads the transcript's media id
from the episode (`/v1/episode/get`), asks `/v1/episode-transcript/get`
for the file and reads its segments. `lang` is the language the file
names, else `und` (not said). An episode without a transcript is
`not_found`. Whether transcripts need a membership is not known. Untested
so far: it needs a login.

### history

`history` lists the episodes the account played, the latest first, as
the app's history page shows them (`/v1/episode-played/list-history`),
each joined with its playback progress (`/v1/playback-progress/list`):
`extra.progress` (seconds), `extra.progress_percent`, `extra.played_at`
and `extra.finished`. It needs a login. `history-delete` and
`history-clear` are not offered. Untested so far: it needs a login.

### login-sms

Logs in with a phone number and a texted code; see `atp guide xiaoyuzhou
login`.

## Limits

- No `search`, charts, discovery or editors' picks feed.
- Subscriptions (`following`) and favorites are not read yet.
- Without a login, `user-posts` sees only the 15 newest episodes a
  show's page lists; older episodes are reachable by their link only.
  Shows that also publish an RSS feed can be read whole with `atp
  podcast episodes FEED`.
- `comments` sees only the hot comments of the page, not every comment.
- Nothing is written: no subscribing, clapping, favoriting or commenting.
