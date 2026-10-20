# Architecture

ATP (all-the-platforms) is one binary (`atp`) that drives social, video and chat platforms through
the same command set. The code is split so that each part stays small and
knows as little as possible about the others.

```
src/main.rs                 registry: lists the platforms, nothing else
src/lib.rs                  the library: atp-core plus each platform crate behind its feature
crates/atp-core          platform-agnostic kernel
crates/atp-<platform>    one crate per platform (atp-zhihu, atp-telegram ...)
crates/atp-meta          Graph API pieces shared by atp-facebook, atp-instagram and atp-threads
crates/atp-bytedance     what Douyin, Toutiao, Juejin and TikTok share: signing, passports, the webcast live protocol
```

Naming, layout and size rules are in [conventions.md](conventions.md).

Dependencies only point downwards: the binary depends on every crate,
platform crates depend on `atp-core` only, and `atp-core` knows no
platform. Adding a platform means adding a crate, one `.platform::<P>()`
line in `src/main.rs`, and its feature (`name = ["dep:atp-name"]`, listed
in `all`) with its re-export in `src/lib.rs`.

The `atp` package is also a library. Its default feature `cli` builds the
binary with every platform; `default-features = false` plus platform
features (`zhihu`, `douyin` ...) gives just those crates, re-exported as
modules next to everything of `atp-core`. `atp_core::open` builds a
platform outside the CLI: the saved session (or cookies given in code),
the same HTTP client and pacing, and `save_session` to keep a refreshed
one. Each platform crate re-exports its `Command` enum for `run_extra`.

## atp-core

| Module | Role |
| --- | --- |
| `platform` | `Platform` trait; `cap` (capabilities), `info` (`PlatformInfo`, `Choices`, login ways), `request` (`PageReq`, `Query`, `Draft`, `Edit`, `Reply`, `Message`, `QrTicket`) |
| `ctx` | `Ctx`: what a platform works with (HTTP client with the session cookies, files, session extras, login hints, `#N` resolution, `--coord` and `--site`) |
| `model` | normalized `Post`, `User`, `Comment`, `Collection`, `Notification`, `FileEntry` (drives), `Page<T>`, `Action`, `AuthStatus`, `Insights`, `Transcript`, and `Data` (everything a command can print), one file per type |
| `cli` | the shared commands, one module per area (`account`, `read`, `library`, `history`, `write`, `follows`, `folders`, `media`, `chat`) flattened into `CommonCommand`; per-platform clap command; global and paging options; questions on the terminal (`prompt`) |
| `app` | top-level program, multi-call (`bili` symlink) |
| `open` | platforms in Rust code: `open`, `Open` (cookies, proxy, timeout, the site of `careers`), `save_session` |
| `doctor` | `atp doctor`: the version, the config and cache folders, who may read each saved session file, the `status` of every saved session (one platform after another), ffmpeg on `PATH`, a Chromium-family browser (`chrome::find`), the proxy in effect, and the network and the clock from one request's `Date`; `--offline` stays local; exit code 1 when a check warns or fails |
| `platforms` | `atp platforms`: one row per platform grouped by kind (TAB marks those read through a browser tab, `Platform::BROWSER_TAB`, and `window` those whose tab opens in a window, `TAB_WINDOW`), `atp platforms <id>` in detail, the same data as JSON (`browser_tab`, `tab_window`) |
| `account` | `login` (QR / token / browser / cookie string), `logout`, `status` |
| `http` | Chrome-fingerprinted client (wreq), cookie jar, pacing (shared by every atp process through `pace`, see [Pacing](#pacing)), proxy; `request` (building, sending, retries, streaming), `response`; a failed request says whether it may have reached a server (`error::Outcome`: `not_sent` only while nothing of the client got past connecting), which commands that change something report as `error.outcome` (`cli::access`) |
| `store` | session file (0600), data files the user keeps beside it (`data`, `put_data`: lists and settings `logout` leaves alone), TTL cache (values and raw bytes), `#N` short-index lists |
| `bulk` | commands on several things (`follow A B C`, `folder-add F P…`): one `Action` per thing in order, `each` one by one and `batches` through a platform's batch call, a failed thing marked and the rest going on, a login / check / rate-limit error ending the run; `move_by_adding`, the default `folder-move` |
| `paging` | `collect`: follow cursors until `--limit` items, dropping repeated items, until the upstream ends or repeats a cursor; `collect_within`: also `--max-pages` (`Bounds`); `collect_window`: `--since` / `--until` |
| `download` | save media, merge DASH audio/video, audio-only, WAV segments (ffmpeg); `files`: a drive's files and folders under their own names, through `.part` files resumed by `Range`, with per-link headers and links renewed mid-file |
| `upload` | files too big for memory: `LocalFile`, `Hashes` (md5, sha1, 4 MB block md5s, md5 of the first 256 KB in one read), `Parts` (part by part with retries, a progress bar and resume state in the private cache; optionally the SHA-1 state before each part, `Sha1Midway`) |
| `task` | background work upstream: `wait` polls a task with growing gaps; `retry` backs off while a call fails with errors a predicate picks (rate limits, risk codes) |
| `oauth` | OAuth device-code logins (`DeviceCode`, `Poll`, `device_login`), the consent page of logins that come back through the browser (`open_consent`) and the one-shot loopback listener they come back to (`port`, `listen`, a random `state`, `wait`), with the parameters in the query or after `#` (`redirect`), and `Tokens` kept in the session extras, saved to disk the moment a single-use refresh token rotates (`access_token`) |
| `file` | format detection by magic bytes, MIME types, files and images to upload |
| `geo` | map platforms: `Coord` (a point in its `System`: `wgs84`, `gcj02`, `bd09`) and the conversions between them, mainland China's outline for Google, `Place` (`Post.place`), `Routes` / `Route` / `Step`, `Distance`, `Polyline` (Google's encoded format), `localize` (every point of a result into the `--coord` system), static map sizes |
| `guide` | `atp guide`: topics from `crates/atp-core/guide/*.md` and what shared commands mean from `guide/commands/*.md`; platform, command (`atp guide bili search`) and login pages generated from the clap command set and `PlatformInfo`, plus each crate's `GUIDE.md` whole or by section; `page` resolves the words for the CLI and MCP |
| `output` | human tables/cards, the `{ok, schema_version, platform, fetched_at, data}` envelope, and flat JSON Lines / CSV rows |
| `json` | `ValueExt`: dotted-path getters tolerant of upstream quirks (`v.str("a.b.0")`, `v.count("stat.like")`) |
| `text` | counts (`1.2万`), durations, truncation, HTML to text, timestamps |
| `feed` | web feeds for platforms that read them (podcasts, RSS): RSS 0.9x–2.0, Atom and JSON Feed through feed-rs into a `Feed` whose entries are posts (enclosures as media, iTunes episode data in `extra`); `get` fetches as feed readers do (a declared client, `If-None-Match` / `If-Modified-Since` against a private cached copy), `load` also follows a web page to the feed it announces (`discover`); `FEED_URL#ENTRY_ID` entry references (`entry_ref`, `split_entry_ref`) |
| `music` | what music platforms share: `Lyrics` (LRC with its translation, as plain text or LRC) and track lists inside a post (`with_tracks`) |
| `chapters` | video chapters: `Chapter` (start in seconds, title), `parse` of `0:00 Title` lines (`--chapters FILE`), `Rules` and `check` (count, first at 0:00, shortest chapter, title length, shortest video), `lines` / `summary`, and `value`, the `chapters` key of `Post.extra` (`[{title, start, end}]`) |
| `jobs` | what job platforms share: `Salary`, the pay of a job read from the text a platform shows (`parse`) or found in a title (`in_text`, conservative), with its figures per `Period` (hour, day, month, year), written into the job keys of `Post.extra` (`fill`) |
| `browser` | import cookies from local browsers (`browser` feature, on by default) |
| `credentials` | environment variables, `login --token` fields, credentials saved in the session, instance hosts (`host`) |
| `token` | `AccessToken`: expiring access tokens cached in memory and in the private cache |
| `chat` | `CHAT/MESSAGE` ids of messages without permalinks |
| `live` | `listen` helpers: polling with de-duplication, `Backoff` (reconnect delays, task polling) |
| `socket` | websocket connections through the same client |
| `proto` | protobuf by hand for binary frames read without a schema: `Msg` (fields on demand) and writers |
| `qr` | login QR codes in the terminal, from a URL or a QR image |
| `chrome` | Chromium-family browsers over DevTools: finding one (`ATP_BROWSER`), starting it on a throwaway profile or a platform's kept one (locked while in use, written out on exit), with or without a window, through ATP's proxy; `cdp`, the protocol: calls on the browser and on a page attached with `flatten`, events kept while calls wait, cookies in and out |
| `tab` | a browser tab for platforms whose API answers a real browser only (`Platform::BROWSER_TAB`, `Ctx::tab`): `fetch` made by the site's own page, `capture` of a request the page makes itself (Network domain), `goto`, `eval`; `Answer::json` turns checks, login walls and pages where data was expected into errors (see [Browser tabs](#browser-tabs)) |
| `verify` | human checks by hand: a browser window with the session cookies (on the platform's own profile for tab platforms), then one retry; skipped for checks a browser cannot clear (`Error::unclearable`, `Platform::BROWSER_CHECKS`) |
| `mcp` | `atp mcp`: JSON-RPC over stdio; tools `platforms`, `help`, `read` (the shared reads and each platform's `EXTRA_READS`), `run` calling commands without printing; the guide as resources (`atp://guide/…`) |

### The Platform contract

A platform crate exports one type implementing `atp_core::Platform`:

- `const INFO: PlatformInfo` declares id, aliases, home page, cookie domains,
  required cookies, supported capabilities (`caps`), accepted option values
  (`choices`), request pacing (`min_interval`), its kind, what logging in
  takes (`token`, `env`) and allows without an account (`guest`), and the
  crate's `GUIDE.md` (`guide`).
- Methods for the shared commands (`search`, `read`, `comments`, `like`,
  `publish`, ...). Each has a default body returning `unsupported_operation`,
  so a platform implements only what it supports and lists it in `caps`;
  unsupported commands are hidden from its `--help`.
- `edit` serves messages and posts alike: `edit_post` takes an `Edit` (text,
  title, topics, cover, landscape cover, visibility, chapters, a new video
  file and its part; `None` keeps a field; `dry_run` asks for the change
  and its requests to be shown, nothing sent) and by default sends a
  text-only change to the chat-style `edit(post, text)`. `Cap::Edit` shows
  the command; `EditTitle`, `EditTopics`, `EditCover`,
  `EditCoverHorizontal`, `EditVisibility`, `EditChapters`, `EditVideo`,
  `EditPart` (the part of a video in several parts) and `EditDryRun`
  show its options. `post --chapters` shows with `PostChapters`, `post
  --category` with `PostCategory` and `post --cover-horizontal` (a 4:3
  cover beside the main one) with `PostCoverHorizontal`; chapters come
  from a file of `0:00 Title` lines (`chapters`).
- Listing methods return one `Page<T>` per call for a `PageReq { cursor, size }`;
  the core follows `next_cursor` until `--limit` is reached. Cursors are opaque
  strings owned by the platform (offset, page number, upstream cursor ...).
- Post / user arguments arrive as strings with `#N` already resolved; the
  platform accepts its own ids and URLs (at least the `url` it puts in
  `Post.url` / `User.url`, because that is what `#N` resolves to).
- `type Extra: clap::Subcommand` holds platform-only commands; `run_extra`
  returns `Data` like everything else. `const EXTRA_READS` names those
  that only read, by name or path (`playlist tracks`); every other one is
  a write (it may change something, keep files or spend a quota).
  `cli::access` tells reads from writes for the shared commands too: the
  MCP `read` tool runs reads only, `atp guide <id> <command>` shows the
  access, and the errors of commands that change something carry
  `error.outcome`. Debug builds check that every declared read names a
  command.
- `const BROWSER_CHECKS: bool`: whether a `verification_required` error
  may be passed in the person's browser and retried with its cookies; by
  default platforms with session cookies (`cookie_domains`) and those
  reading through a browser tab. Single errors opt out with
  `Error::unclearable()`.
- `const BROWSER_TAB: bool`: the platform reads through a browser tab
  (`Ctx::tab`, see [Browser tabs](#browser-tabs)); off by default.
  `const TAB_WINDOW: bool`: that tab opens in a small window by default,
  for sites whose checks turn away a browser without one.
- `follow_ref` picks the `#N` list `follow` takes from: users by default,
  chats or communities where following joins one.
- `token_whoami` is the account `login --token` checks: `whoami` by
  default; platforms whose `whoami` prefers a cookie session check the
  token instead.
- `const COORD: Option<geo::System>`: map platforms name the coordinate
  system they work in; it is the default of the `--coord` option they get.
- `const SITE: Option<&str>`: a platform reaching many sites through one
  crate (companies' career sites, `careers`) gets a global `--site`
  option with this help; its value is `Ctx::site`.
- `const FOLLOWING_SORT` / `FOLLOW_GROUP_SORT`: the values of
  `following --sort` and `follow-group --sort` (`recent`, `oldest`,
  `smart`, `visited`); empty hides the option. `following_with` takes
  the `FollowQuery` (`--sort`, `--search` behind `Cap::FollowingSearch`,
  `--mutual` behind `Cap::FollowingMutual`) and falls back to
  `following` when it is empty.
- Several posts or users at once: `like_posts`, `favorite_posts` and
  `follow_users` (by default `like`, `favorite` or `follow` one by one;
  platforms with a batch call override them).
- Managing follows and folders: `remove_followers`, `block`, `blocked`,
  the follow groups (`follow_groups`, `follow_group`,
  `follow_group_create` / `rename` / `delete` / `add` / `remove`) and the
  favorites folders (`folder_create` / `edit` with a `FolderSpec`,
  `folder_delete`, `folder_add`, `folder_remove`, `folder_move`, by
  default add then remove). Methods on several users, posts or folders return one Action
  per item (`Data::Actions`; one item prints a single Action). Each
  command has its `Cap`; `FolderDescription`, `FolderPrivacy` and
  `FolderCover` show the options of `folder-create` / `folder-edit`.
- Watch later and the history: `watch_later` (a listing like `history`),
  `later_add` / `later_remove` (one Action per post), `later_clear`
  (`Cap::LaterClearWatched` shows `--watched`, only the posts already
  watched), `history_delete` (one Action per post) and `history_clear`,
  behind `WatchLater`, `LaterAdd`, `LaterClear`, `HistoryDelete` and
  `HistoryClear`. The CLI asks before `history-delete`, `history-clear`
  and a `later-clear` without `--watched`.

### Chat platforms

Messaging platforms use the same model: a chat (server, channel, group,
direct conversation, thread) is a `Collection` and a message is a `Post`. The
chat commands (`chats [PARENT]`, `messages CHAT`, `send CHAT`, `members`,
`react`, `edit`, `listen`) map to trait methods behind `Cap::Chats`,
`Messages`, `Send`, `Members`, `React`, `Edit` and `Listen`, and stay hidden
on platforms without them. `delete`, `read`, `search`, `user` and `download`
work on messages as on posts. A message without a permalink takes
`CHAT/MESSAGE` as its `id`, so it can be passed back like a link; `#N`
resolves chats and messages from the last list. `listen` hands each event to
an `output::Stream`, which prints it at once (a line, or one JSON object /
YAML document per event).

Credentials that are not cookies (bot tokens, API keys, `AppID:Secret`) come
in through `login --token` (`Cap::TokenLogin`, `Platform::login_token`) and
are kept in the session extras, or from environment variables.

### Drives

Cloud drives (`Cap::Files`) list `FileEntry`s (id, name, path, folder or
file, size, time, md5, share link) where other platforms list posts. The
shared `search`, `read`, `download` and `delete` call `search_files`,
`file`, `files` + `file_link` and `delete` instead of the post methods;
`#N` after a listing refers to an entry's `id`. `download::files` walks a
folder through `files`, saves under the drive's names and asks
`file_link(file, renew)` for a link (with the headers its host wants)
again when the host refuses one mid-file; session cookies go to a link
only when it lists them, and never across sites on redirects. Uploads use
`upload::Parts`, background operations `task::wait`, and drives with an
open platform `oauth`. Everything else (`ls`, `mkdir`, `mv`, shares, the
recycle bin) is the drive's own `Extra` commands.

### Map platforms

Map platforms (`amap`, `gmap`, `bmap`, `qqmap`) share command names and
options: places through `search`, `read` and `download`, and their own
`nearby`, `tips`, `geocode`, `regeo`, `route`, `distance` and `staticmap`
with the same arguments wherever the APIs allow. A place is a `Post` with a
`geo::Place` (point, address, category, phone, rating, distance, region);
`route` returns `Data::Routes` (routes with distance in metres, duration in
seconds, steps and a polyline) and `distance` returns `Data::Distances`.
Tables, cards and jsonl/csv rows know these shapes.

Every coordinate carries its `geo::System`. `Platform::COORD` gives a
platform a global `--coord` option defaulting to it; the CLI puts its value
in `Ctx::coord`. The platform reads user points in that system and converts
them to what its API takes (Amap and Tencent GCJ-02; Baidu BD-09 or, through
`coord_type` / `ret_coordtype`, GCJ-02; Google WGS-84, or GCJ-02 inside
mainland China), and tags the points it parses with the system the API
answered in. After the command, `geo::localize` converts every typed point
(places, routes, distances) and every `{lat, lng, system}` object in values
and `extra` into the `--coord` system.

### Browser tabs

Some sites answer their own JSON API only to a real browser: a bot check
in front of it runs the site's scripts (Aliyun's WAF on 51job), or the
requests carry a token the site's pages compute (BOSS Zhipin's
`__zp_stoken__`). ATP never ports or runs such token code itself, in
Rust or in an embedded engine. A platform declaring `BROWSER_TAB` reads
through a browser instead:

- `Ctx::tab()` starts, on first use, a Chromium-family browser of the
  person's machine (the one `verify` finds; `ATP_BROWSER` names another)
  without a window (`--headless=new`), with DevTools on a local port.
  Without a window Chrome names itself HeadlessChrome; sites whose checks
  turn that away declare `TAB_WINDOW`, and their tab opens in a small
  window instead: the same browser with its usual user agent, nothing
  hidden, moved or minimised. Window or not, Chrome driven over DevTools
  tells pages it is automated (`navigator.webdriver`), and ATP leaves it
  so. `--browser-window never|auto|always`
  (`ATP_BROWSER_WINDOW`, `Open::browser_window`, `tab::Window`) overrides
  the platform's choice; without a display (`DISPLAY`,
  `WAYLAND_DISPLAY`) the tab runs without a window, says so, and a check
  it then meets says so in its hint (`tab::NO_DISPLAY`).
  It runs on the platform's own profile, `<cache>/<platform>/browser`,
  kept between runs so the site's cookies, storage and device ids last;
  another atp using it at the same time gets a throwaway profile
  instead. The profile is seeded with the session's cookies it lacks
  (the login's always), and the cookies the site sets come back into the
  session after each step, as `verify` does. The proxy is ATP's.
- One page is attached over DevTools. `Tab::fetch(page, url, init)` opens
  a page of the site (once per origin) and runs `fetch(url, {credentials:
  'include', …init})` in it: the site's own scripts have run, the
  request is same-origin and carries the page's cookies. `Tab::capture`
  opens a page (or runs a call of the page's own code) and returns the
  answer to the first request whose URL matches, read from the Network
  domain (`getResponseBody`, 16 MB at most). `goto` and `eval` open a page
  and read from it.
- Nothing is disguised: no other user agent, no automation marks hidden,
  no scripts added to pages, no `Runtime.enable`. Requests through the
  tab (page loads, fetches) keep the platform's `min_interval`.
- A page that loads a check waits for the page to pass it by itself (a
  script that sets a cookie and reloads); a check still there, or an
  answer that is one, is `verification_required` with the page.
  `Platform::BROWSER_CHECKS` is on for tab platforms, so on a terminal
  the CLI opens that page in a window on the same profile for the person
  to pass, then runs the command again. ATP never clicks, drags or solves
  a captcha or slider. HTML where JSON was expected is never parsed as
  data: `Answer::json` makes it `verification_required` (known checks),
  `not_authenticated` (a 401 or a redirect to a login page) or
  `upstream_error`.
- The CLI ends the browser after each command (`Ctx::close_tab`, which
  keeps its cookies in the session first); dropping the context ends it
  too, giving the browser a few seconds to write its profile out.
  `logout` deletes the profile.

### Pacing

Each platform declares `min_interval`, the least time between two of
its requests (`--interval` / `ATP_INTERVAL` can only raise it). `Http`
keeps it within a process (from the end of its last request, plus a
little jitter and now and then a longer pause) and across processes
through `<cache>/<platform>/pace`: a 21-byte file holding the start the
latest request of any atp process reserved, in milliseconds since the
epoch. Before a request, `Http::throttle` takes an advisory lock on the
file (`File::try_lock`, which is `flock` on Unix and `LockFileEx` on
Windows), reads that start, reserves its own (the later of its own
earliest start and that one plus `min_interval` and jitter), writes it,
unlocks and sleeps until then: one small write per request, the lock
held only for the read and the write. Requests through a browser tab
reserve their start the same way; `no_throttle` requests (media, drives'
files) and platforms without a `min_interval` skip it. When the file
cannot be opened or stays locked for 50 ms (ten tries), or locks are not
supported, the request is paced by its process alone. A reservation more
than ten minutes ahead counts as a clock that jumped and is ignored.
`atp_core::open` paces through the same file; an `HttpConfig` without
`pace` paces alone.

### Flow of one command

1. `app` parses arguments, picks the platform, starts a current-thread tokio runtime.
2. `cli::execute` loads the session (`ATP_<ID>_COOKIE` wins over the saved
   file), builds `Http` with those cookies and calls `P::new(ctx)`.
3. The shared command maps to a trait method (listings go through `paging::collect`);
   on map platforms the points of the result are converted to the `--coord` system.
4. A browser tab the command opened ends; cookies changed by responses
   (and the tab) are written back to the session file (logged-in
   sessions only).
5. Lists are remembered for `#N`; `raw` payloads are dropped unless `--raw`;
   `output` prints a table or the JSON/YAML envelope.

## Inside a platform crate

Every platform follows the same layout so each file has one job:

| File | Role |
| --- | --- |
| `lib.rs` | `PlatformInfo` and the `Platform` impl: thin, delegates to the modules below |
| `api.rs` | the platform's transport: base headers, signing hook, response envelope → `Error` |
| `sign.rs` (or `sign/`) | request signatures (WBI, x-s, x-client-transaction-id, SAPISIDHASH ...) |
| `refs.rs` | parse ids and URLs into typed references |
| `parse.rs` | upstream JSON → core models |
| domain modules (`read.rs`, `write.rs`, `account.rs` ...) | endpoints grouped by domain |
| `extra.rs` | the `Extra` subcommand enum and its handlers |

Rules of thumb:

- Parse upstream JSON with `ValueExt` instead of mirror structs; keep the
  untouched object in the model's `raw` field.
- Map upstream failures to the shared `ErrorCode`s (login → `not_authenticated`,
  captcha → `verification_required`, ...), so scripts can react the same way everywhere.
- Long-lived upstream metadata (signing keys, GraphQL query ids) goes into
  `ctx.store.cache_get/cache_put` with a TTL.
- Write operations pause briefly (`ctx.http.pause`) where the platform is known
  to flag bursts.
