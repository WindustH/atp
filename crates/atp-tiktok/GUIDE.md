## Logging in

`atp tiktok login` shows a QR code: in the TikTok app open Profile → ☰ →
the scan icon, scan it and confirm. The session cookies arrive with the
confirmation. When TikTok confirms the scan but sets no session, the login
fails with `not_authenticated`; when TikTok rate-limits the QR login, with
`rate_limited` (wait a while). Use `--browser` or `--cookie` then.

`login --browser` reads the session of a browser logged in at tiktok.com.
With `--cookie`, copy the whole `Cookie` header of a request to
`www.tiktok.com`: `sessionid` is the session, writes also send
`tt_csrf_token`, and API calls reuse the `msToken` cookie when there is
one.

`login --token ACCESS_TOKEN` keeps an access token of the official
Marketing API with the session, for `ads` only: a long-term token of a
Marketing API app from https://business-api.tiktok.com (a token has no
spaces). It does not log in to tiktok.com. A session with a token and no
cookies counts as logged in: `login` checks the token by asking whom it
belongs to (also when the session has cookies, which it leaves alone),
and `whoami` shows that person (`source: marketing-api`) when there are
no cookies.
Listing the ad accounts a token manages also needs the app's id and
secret (`TIKTOK_ADS_APP_ID`, `TIKTOK_ADS_SECRET`).

### What works without a login

Pages: `read` (videos and photo posts, with exact counters, the stream
ladder and the sound), `user`, `insights` of a post, and `download`.
`live` and `listen` read live rooms without a login too.
TikTok puts a proof-of-work check in front of pages now and then; it is
solved like the browser does.

Lists come from the web client's signed API (`X-Bogus`, `msToken`):
comments, a user's videos, search, the recommended feed, followers,
hashtag / sound / playlist videos. TikTok scores these calls and often
answers anonymous ones with nothing (`verification_required`); a logged-in
session from a residential network fares best. Newer web pages also sign
with `X-Gnarly`, which is not reproduced.

Your own likes, favorites, collections and playlists (without a USER),
`blocked`, `saved-music`, account `insights`, `notifications`, `studio`,
and every write (`like`, `favorite`, `comment`, `delete-comment`,
`follow`, `block`, `approve-follower`, `save-music`, `delete`, `post`,
`edit`, the `folder-*` and `playlist-*` commands) need a cookie session.
`post`, `edit` and the playlist writes also need a Chromium-family
browser on this machine (see `post` and `playlist-create`).

### When the session ends

How long a session lasts is not known. When TikTok stops accepting it,
commands fail with `not_authenticated` ("TikTok did not recognize the
session", "TikTok wants a login for this"); log in again. An invalid or
expired Marketing API token fails `ads` with `not_authenticated` as well.

### Bot checks

- When TikTok keeps serving its proof-of-work page, the command fails with
  `verification_required`: open tiktok.com in a browser, or retry later /
  through `--proxy`.
- An empty answer from the API, or TikTok's codes 10000 / 10101, also mean
  `verification_required`: log in (`--browser`), or retry later / from
  another network (`--proxy`).
- A human check in front of TikTok Studio's page (`post`, `edit`) is
  `verification_required` too; on a terminal ATP opens that page in a
  window of its browser, on TikTok's profile, for you to pass, then runs
  the command again. ATP never solves a check itself.

### Environment

- `TIKTOK_ADS_ACCESS_TOKEN`: the Marketing API access token; it wins over
  the one `login --token` saved.
- `TIKTOK_ADS_APP_ID`, `TIKTOK_ADS_SECRET`: the Marketing API app's id and
  secret, for `ads advertisers`.
- `TIKTOK_ADS_ADVERTISER_ID`: the ad account to use when `-a` is not
  given (before the one `ads use` saved).
- `ATP_TIKTOK_COOKIE`: a cookie header for one run, never saved.

## References

- Videos and photo posts: `https://www.tiktok.com/@user/video/<id>`,
  `…/photo/<id>`, short links (`https://vm.tiktok.com/…`,
  `https://vt.tiktok.com/…`, `https://www.tiktok.com/t/…`) or a bare id.
- Users: `tiktok`, `@tiktok`, a profile link (short links too), or a
  secUid (`MS4wLjAB…`).
- Hashtags: `#name`, `name` or a `/tag/` link.
- Sounds: the id or a `/music/<name>-<id>` link.
- Playlists: the id or a `/@user/playlist/<name>-<id>` link.
- Favorites collections: the id or a `/@user/collection/<name>-<id>` link.
- Comments: the comment id.
- Live rooms (`listen`): the host's `@handle`, `live:@handle`, a
  `https://www.tiktok.com/@handle/live` link, or the room id; `live`
  takes the host, not a room id.

## Commands

### search

`--filter general` (the Top tab, the default) or `--filter video` (videos
only). `-t user` finds accounts. `-t topic NAME` looks up that exact
hashtag, with its video and view counts (the web search has no hashtag
tab).

### hot, feed

Both read the recommended "For You" feed, personal when logged in. It has
no end; any cursor continues it.

### read

Videos and photo posts from their page, with exact counters, the stream
ladder and the sound; no login needed.

### likes

Videos a user liked, public only when the user allows it; yours without a
USER.

### favorites, collections

`favorites` lists saved videos (yours without a USER); `--folder ID` lists
one favorites collection (its id or link). `collections` are the favorites
folders.

### favorite, unfavorite

Saves to (or removes from) your saved videos; `--folder` is not supported
(file a saved video with `folder-add`).

### folder-add, folder-remove, folder-move

Your favorites collections, as the collection page's "Add videos",
"Remove" and "Move" do; FOLDER is a collection id, link or `#N` after
`collections`. All video arguments are read first.

- `folder-add COLLECTION VIDEO…` files saved videos into a collection and
  `folder-remove` takes them out (they stay saved), 20 per call. Save a
  video first with `favorite`.
- `folder-move FROM TO VIDEO…` uses TikTok's own move, 20 per call;
  `--copy` adds them to TO and leaves FROM as it is.
- These calls also carry the `x-cthulhu-csrf: 1` header the page's CSRF
  script puts on them.

### folder-create, folder-edit, folder-delete

The collections themselves, as the Favorites tab's "Create new
collection" and a collection's menu ("Make public", "Edit name",
"Delete") change them. FOLDER is a collection id, link or `#N` after
`collections`.

- `folder-create NAME` makes one (`/api/collection/create/` with
  `collectionName` and `collectionStatus`: 3 public, 1 with
  `--private`). The page's form starts private; ATP's `folder-create` is
  public unless `--private`, as on other platforms. A public collection
  shows on your profile.
- `folder-edit FOLDER` renames it (`--name`) or makes it `--public` /
  `--private` (`/api/collection/modify_info/`). The menu sends the name
  and the visibility every time, so ATP reads what is not given from
  your collection list (`/api/user/collection_list/` with
  `publicOnly=false`, as the page asks for its own).
- `folder-delete FOLDER…` deletes them, one call each, after asking; the
  videos stay saved.
- Names take 1 to 30 characters ("Character limit exceeded"). Before it
  names a collection the page runs TikTok's name check
  (`/api/playlist/name_check`: "This name is inappropriate"), and so does
  ATP. Collections have no description or cover on the web.
- The parameters go in the query with an empty body, as for
  `folder-add`; they are signed like the other writes (`X-Bogus`) and
  carry the `x-cthulhu-csrf: 1` header. The page also sends `X-Gnarly`,
  which ATP does not make; whether TikTok wants it on these calls is not
  known.

Untested: built from the profile and collection pages' scripts; nothing
was sent to TikTok, and the reads before sending were not run against an
account either.

### block, unblock, blocked

`block USER…` / `unblock USER…` as the profile menu does, one call per
user (each looked up for its secUid first). `blocked` lists the accounts
you block, as Settings → Privacy → Blocked accounts does
(`/api/block/user_list/`, 20 a page, signed with `X-Bogus` like the
other lists; whether TikTok asks it for more is not known). TikTok's web
has no way to remove a follower, so there is no `remove-follower`.
`blocked` is untested so far: it needs a login.

### approve-follower

`approve-follower USER…` lets people who asked to follow your private
account follow it, as the inbox's Accept does
(`/api/commit/follow/request/approve/` with their user id), one signed
POST per user, sent like `follow`. The list of requests is not read
(find them in the inbox of the app or the web), and declining is not
offered. Untested so far.

### comment, delete-comment

`--reply-to COMMENT` takes the id of the comment to answer.

### delete

Deletes one of your posts.

### post

`post --video FILE` publishes a video, and `post -i PHOTO…` a photo post
(up to 35 JPEG, PNG or WebP photos of 50 MB at most, with an optional
`--title` of up to 90 characters), the way TikTok Studio's upload page
(`/tiktokstudio/upload`) does. TEXT is the caption, 4000 characters at
most. `--topic NAME` appends `#NAME` (one word); `#words` in the text are
hashtags as well. A typed `@name` stays plain text (a linked mention
needs the account's id, which the editor picks from its suggestions).

- `--visibility public|friends|private` (everyone, friends, only you).
- `--ai` sets TikTok's AI-generated content label.
- `--schedule WHEN` publishes 15 minutes to 10 days ahead (TikTok allows
  30 days on some accounts, and refuses past what it allows), to the
  minute. TikTok holds at most 30 scheduled posts.
- `--collection PLAYLIST` adds the post to one of your playlists: its id,
  link or exact name, as `playlists` lists them. The post must be public.
- Comments, duets and stitches are allowed, as Studio's form has them by
  default; videos over a minute get no duets, stitches or reuse, as
  Studio sets them.
- A video has no title (`--title` is for photo posts). Studio takes a
  video's cover from its frames: `--cover` is refused, and TikTok shows
  the first frame.
- Not offered: `--location` (Studio's place anchor needs a place search
  ATP does not have), music, chapters, `--reply-to` / `--quote`.

ATP shows what it is about to publish and asks before uploading anything;
scripts pass `-y`. The upload keys come from `/api/v1/video/upload/auth/`;
the files go to ByteDance's upload gateway at `www.tiktok.com/top/v1`
(videos to the VOD space `tiktok`, photos to the ImageX service
`photomode`), signed with those keys (AWS Signature V4) for the data
region Studio's page names. TikTok then transcodes a video; ATP waits for
that as the page does (`transcode/result`, up to an hour), then creates
the post (`/tiktok/web/project/post/v1/`) with the fields Studio's form
sends. The result has the new post's `id` and `url`; when TikTok answers
without one (it is still working on the post), the Action says so and
`studio` lists the post once it is up. A video upload cut off by the
network continues on the next run (for two hours).

How the calls are signed: Studio's pages sign the upload keys, the post
and edits with tokens their own scripts compute (`byted_acrawler`'s
`_signature`, the ZTI SDK's ticket-guard headers and `X-Gnarly`, besides
`X-Bogus`). ATP does not compute them. Those calls are made by Studio's
page itself, open in ATP's browser tab: a Chromium-family browser of this
machine (`ATP_BROWSER` names another) in a small window
(`--browser-window never` runs it without one, which TikTok's scripts see
as HeadlessChrome), on TikTok's own profile, with the session's cookies.
ATP waits until the page has signed a request of its own, then has it
send the call; the page's scripts add what they add. Everything else goes as
the other calls do: the upload gateway with its keys, the transcode calls
and the post's details signed with `X-Bogus`.

Untested: publishing has not been tried. Uploading creates objects on
TikTok even without a post, and no session was at hand, so neither a post
nor a read of Studio's pages has been run. What ATP sends was read from
the scripts of Studio's upload page (web-creation 1.0.0.6391) in October
2026. Tried without a login: in ATP's browser tab, tiktok.com's scripts
replace the page's `fetch` and add `msToken`, `X-Bogus`, `X-Gnarly` and
`X-Dynosaur` to what it sends, and Studio's pages show a browser without
a session the login page. Whether TikTok accepts these calls from a
browser driven over DevTools (it tells pages it is automated) is not
known.

### edit

`edit POST` changes one of your posts as Studio's edit page does
(`/tiktok/post/edit/v1/`, made by Studio's page like `post`). TEXT
replaces the caption (its `#words` are hashtags; the @-mentions the
caption had stay linked where the new text keeps their `@name`).
`--topic NAME` replaces the caption's hashtags: TEXT, or the caption
without its hashtags, then the new ones; `--no-topics` removes them.
`--title` changes a photo post's title, `--visibility` who can see the
post.

ATP reads the post first (`/api/v1/post/detail/`, with what Studio lets
change now), shows each change as old → new and asks; scripts pass
`-y`. When Studio says a field cannot change now (one edit a day, only
within some days of posting, an earlier edit still in review, a
scheduled post's visibility ...), ATP shows why and sends nothing.
`--dry-run` shows the change, what Studio allows and the request, and
sends nothing.

Not offered: a new video (TikTok keeps a post's video), chapters, a new
cover (Studio's cover editor is not reproduced), and the comment, duet
and stitch switches (no shared option has them).

Untested: editing has not been tried, and neither has the read of a
post's details (it needs a login).

### tag

`tag NAME`: the videos of a hashtag.

### sound

`sound ID`: the videos using a sound; `--info` shows the sound itself.

### playlist

`playlist ID`: the videos of a creator's playlist; `--info` shows the
playlist itself.

### playlists

`playlists [USER]`: the playlists (mixes) a creator made of their videos;
yours without a USER, in your profile's order. `playlist-create` and the
commands below make and change yours.

### playlist-create, playlist-edit, playlist-delete, playlist-add, playlist-remove, playlist-sort, playlists-sort

Your playlists, as the profile's Playlists tab ("Create a playlist",
"Change playlist order") and a playlist's page ("Add posts", "Remove
posts", "Edit playlist's name", "Delete playlist", dragging posts) keep
them. PLAYLIST is one of yours: its id, link, exact name or `#N` after
`playlists`. VIDEO is a post id, link or `#N` (`studio` lists yours).
Each command reads what it changes, prints the change and asks (`-y` in
scripts).

- `playlist-create TITLE VIDEO…` makes one of the posts given, in that
  order (`/api/playlist/create_with_items/`, `name` and `itemIds`); the
  page creates none without a post.
- `playlist-edit PLAYLIST --title NAME` renames it
  (`/api/playlist/update/`). Playlists have no description or cover on
  the web.
- `playlist-delete PLAYLIST` deletes it (`/api/playlist/delete/`); it
  cannot be recovered, its posts stay.
- `playlist-add PLAYLIST VIDEO…` adds posts
  (`/api/playlist/modify_items/` with `addItemIds`); the page shows added
  posts first. `playlist-remove` takes posts out (`deleteItemsIds`); they
  stay on your account.
- `playlist-sort PLAYLIST VIDEO…` puts the posts given first, in that
  order, and the others after them as they were. Like the page's
  dragging, it sends the new order up to the last post that moved
  (`commitItemIds`).
- `playlists-sort PLAYLIST…` does the same with the order of your
  playlists on your profile (`/api/playlist/modify_order/`,
  `commitMixIds`).

TikTok's rules, as the web client holds posts and accounts to them:

- TikTok decides per account who may make playlists: your profile shows
  "Create a playlist" only when its `canExpPlaylist` is true, and ATP
  refuses `playlist-create` when it is false (`permission_denied`). What
  earns it is not said in the web client.
- Only your own public posts join ("Only public posts can be added to
  playlists"): ATP refuses others' posts and private or friends-only ones
  (`privateItem`, `secret`, `forFriend` of the post).
- A post is in one playlist at most ("Each post can only be in 1 playlist
  at a time"): ATP refuses a post whose `playlistId` names another
  playlist; take it out there first.
- Names take 1 to 25 characters and must pass TikTok's name check
  (`/api/playlist/name_check`, "This name is inappropriate"), which ATP
  runs before naming one, as the page does.
- 100 posts: the page picks no more for a new playlist and adds none past
  100 ("You can select up to 100 posts at a time"); ATP holds to that.
  Whether TikTok itself caps a playlist is not known.
- TikTok may refuse on its own: 206001 "Couldn't create playlist" and
  206003 "Couldn't add post to playlist" are `permission_denied`; 206002
  creates the playlist but shows it to you only for now.

These calls carry a form body, and ATP's `X-Bogus` signs a query over an
empty body only, so TikTok's own page makes them. ATP opens your profile
in a browser tab (a Chromium-family browser of the machine, `ATP_BROWSER`
names another; a small window, `--browser-window never` runs it without
one), waits until the page has signed a request of its own, and has the
page `fetch` the call. The page's signing script adds `msToken`,
`X-Bogus` and `X-Gnarly`; ATP adds what the page's request library adds:
the device block (built in the page from its app context), the context's
`tt-csrf-token` for create and rename, and `x-cthulhu-csrf: 1` for create,
delete and `playlists-sort`. Driven over DevTools the browser tells the
page it is automated (`navigator.webdriver`); whether TikTok then turns
these writes away is not known. The reads around them (your profile, its
playlists, a playlist's posts, each post, the name check) are ATP's usual
signed API calls. TikTok Studio has playlist calls of its own
(`/tiktok/v1/creator/playlist/create/`, `…/manage`) in its post editor;
ATP uses the profile's.

Untested: built from the profile and playlist pages' scripts; nothing was
sent to TikTok, and the checks before sending were not run against an
account. Read without a login: `playlists USER`, `playlist ID` and
`playlist ID --info` of a public creator, the reads these commands build
on.

### related

`related VIDEO`: the videos TikTok recommends next to one.

### saved-music, save-music

Your saved sounds (Favorites → Sounds), as TikTok Studio's sound picker
lists them (`/api/user/collect/music_list/`, signed with `X-Bogus`),
latest saved first. `save-music SOUND…` saves sounds and `--undo`
unsaves them, one call each (`/api/music/collect/`, a signed POST like
`favorite`); SOUND is an id, a `/music/` link or `#N` after
`saved-music`. TikTok's web cannot save playlists, so there is no list
of saved playlists. Untested so far: both need a login.

### insights

The public counters: for a post views, likes, comments, shares,
favorites, reposts and the like rate; for your account followers,
following, likes, posts and friends. TikTok Studio's analytics pages
(traffic sources, audience, watch time) are not read; its content list
is, with each video's counters: see `studio`. Ad accounts have their
reports in `ads report`.

### studio

`studio` lists your videos as TikTok Studio's content page does
(`/tiktokstudio/content`), the newest first, each with the counters
Studio shows: views, likes, comments, saves (`metrics.favorites`) and
shares. `-n` follows Studio's pages (50 videos each); `--since` /
`--until` keep a window; `-f csv` gives one row per video. It needs a
cookie session, and an account with TikTok Studio. The call is signed
like the other API calls (`X-Bogus` over the query); whether TikTok
asks this route for more than that is not known. Untested so far: it
needs a login.

### notifications

`notifications [--type all|like|comment|mention|follower]` reads the
inbox's activity (`/api/notice/multi/`), the newest first: who liked,
commented, mentioned or followed, the video it was about (`target`,
`url`) and whether it was unread. Whether reading them marks them read
is not known. Untested so far: it needs a login; notice kinds ATP does
not know come as `other`.

### ads

The official Marketing API, with the token of `login --token` or
`TIKTOK_ADS_ACCESS_TOKEN`; no cookie session needed. The commands that
work on one ad account take `-a ID`, else `TIKTOK_ADS_ADVERTISER_ID`, else
the one `ads use ID` saved. Lists come as rows, one per object (`-f csv`
for spreadsheets); `-n` caps them (100 by default, 0 for all).
`--status` keeps the campaigns, ad groups or ads of one primary status.

### ads me, ads advertisers, ads advertiser, ads use

`me` is the person the token belongs to. `advertisers` lists the ad
accounts the token manages (needs `TIKTOK_ADS_APP_ID` and
`TIKTOK_ADS_SECRET`). `advertiser [ID…]` shows details (the current
account by default). `use ID` saves the default ad account with the
session.

### ads campaigns, ads adgroups, ads ads, ads audiences, ads pixels

Campaigns, ad groups (`--campaign ID`, repeatable), ads (`--campaign ID`,
`--adgroup ID`), custom and lookalike audiences, and pixels (the first 20)
of the ad account.

### ads report

`ads report` gives one row per dimension combination, dimensions then
metrics. `--level` is `campaign` (the default), `adgroup`, `ad` or
`advertiser`. The dimensions default to `<level>_id,stat_time_day`. The
window is `--from … --to …` (YYYY-MM-DD), or the `--days N` days (7)
up to yesterday. `--type` picks `BASIC`, `AUDIENCE`, `PLAYABLE_MATERIAL`
or `CATALOG`, `--filter` takes the API's filtering JSON. Dimension and
metric names are the Marketing API's (`report/integrated/get`).

### ads raw

`ads raw PATH key=value …` calls any other Marketing API endpoint (the
path after `/open_api/v1.3`); values that parse as JSON are sent as JSON,
`--post` sends a JSON body.

### live

`live @HANDLE`: the host's live room from the live page's call
(`/api-live/user/room/`, sent without a login or a signature): the
title, the host, when it started, `extra.live`, `extra.room_id` and
`extra.viewers_total` (viewers in all); while it is on air also
`extra.viewers` (now) and `extra.streams`, the FLV and HLS addresses by
quality (`origin`, `uhd`, `hd`, `sd`, `ld`; `ao` is audio only). A host
who is not live comes with `extra.live` false and the title, start and
room of the last live. Tested on hosts whose live had ended; a room on
air is untested.

### listen

`listen @HANDLE …` follows live rooms over TikTok's webcast websocket,
without a login. Each event is a post whose `kind` is `message` (a
comment), `gift` (name × count, `extra.count`), `like` (`extra.likes`,
`extra.likes_total`), `enter` (someone joined, `extra.audience`),
`follow`, `share`, `stats` (viewers now, `extra.viewers`, and in all,
`extra.viewers_total`; one every 30 s at most) or `offline` (the live
ended, which ends the stream); `extra.chat` is `live:@handle`. The first
events may be the room's latest few comments. A host who is not live now
is `not_found`; a dropped connection is opened again.

The connection carries the web SDK's `frontierSign`, computed the way the
page does (no signing service is involved); TikTok does not check it
today.

### download

The highest-resolution stream without watermark (HEVC when TikTok has
it), photo posts' images and their sound; `--audio-only` keeps the audio.
Streams are bound to the `tt_chain_token` cookie the post's page set; ATP
sends it along. No login needed.

## Limits

- TikTok's web keeps no watch history of videos (only one of short
  dramas), so there is no `history`.
- Requests are at least 1.5 s apart; writes wait another 1.5–4 s, as a
  person would.
- TikTok's rate limit ends a command with `rate_limited` ("TikTok is rate
  limiting"); wait a while.
- `post`, `edit` and the playlist writes run TikTok's pages in a browser
  tab, which a Chromium-family browser on the machine must provide; the
  tab keeps its own profile (`logout` deletes it). `post` uploads in
  parts of 3 to 10 MB, outside the request pacing.
