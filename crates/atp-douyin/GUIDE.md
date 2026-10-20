## Logging in

### QR code

`atp douyin login` shows a QR code of Douyin's web passport
(login.douyin.com). Scan it with the Douyin app and confirm. When the
passport refuses this client (error 4031, "网站存在安全风险", or a page
instead of data), the login stops with `verification_required`; passing a
check in the browser does not help there, so log in with `--browser` or
`--cookie` instead (reads only, below).

### Phone and texted code

`atp douyin login-sms [PHONE]` asks Douyin's passport to text a code to
the number (mainland numbers as they are, others as `+CC NUMBER`; asked
when left out), then asks for the code on the terminal. When the passport
wants a captcha first (a slider or an identity check), the login stops
with `verification_required` and its page; ATP opens it in your browser,
and after you pass it the login runs again (with `--no-verify` the page is
only printed). It needs a terminal; ATP never reads texts itself.

### Write key

Before either login ATP creates a key pair and hands the public key to the
passport, as the browser does (Douyin's "ticket guard"); the passport
answers the login with a ticket for that key, which ATP keeps with the
session. The writes the web client guards with it need that key, so they
work only on sessions made by these two logins: likes and taking several
likes back, saving a post and unsaving one, comments, follows, approving
follow requests, creating, changing and deleting folders and moving posts
in and out of them, saving mixes, short dramas and music, publishing,
editing and deleting. Whether the passport issues that ticket to ATP's
logins has not been confirmed; without it, these writes fail with
`not_authenticated` ("this session cannot sign writes").

The web client sends the other writes without the key, and so does ATP:
`remove-follower`, `block` / `unblock`, `unfavorite` and `save --undo` /
`save-drama --undo` with several at once, watch later (`later-add`,
`later-remove`, `later-clear`), `history-clear` and the collection
commands (`mix-create` … `mix-sort`). They need a logged-in session of
any kind, `--browser` and `--cookie` included.

### Browser or cookie

Log in on douyin.com in a browser, then:

- `atp douyin login --browser` reads that browser's cookies.
- `atp douyin login --cookie '...'`: copy the `Cookie` header of a request
  to www.douyin.com. It must hold `sessionid`; the `UIFID` cookie a browser
  receives is needed for most reads (below).
- `ATP_DOUYIN_COOKIE='...'` passes such a header for one run; it is never
  saved.

Such sessions read everything a login allows but cannot make the writes
guarded by the write key (above): the browser keeps its key to itself.

There are no other environment variables and no tokens.

### Without an account

Without a login, the hot boards (`hot`), user profiles (`user`) and live
rooms (`live`, `listen`) answer, and `read` and `download` fall back to the
share page's API (iesdouyin.com), which answers visitors. Everything else
keyed by a post or a list of posts (`user-posts`, `likes`, `mix`, `music`,
the recommended `feed` ...) needs the `UIFID` cookie a browser receives (a
logged-out browser's cookies work too, passed as `ATP_DOUYIN_COOKIE`);
without it Douyin answers "Uifid Not Found" and ATP reports
`not_authenticated`. Search, the following and friends feeds, followers,
following, `@douyin_id` lookups and the account's own lists
(`collections`, `favorites`, `history`, `notifications`, your `likes`)
need a login, and writes need ATP's own login (above). So do the creator
platform's reads (`insights`, `works`, `drafts`, `poi`, `hotspots`,
`activities`, `search -t topic`), with a login of any kind.

### When the session ends

How long a session lasts is not known. When it ends, commands fail with
`not_authenticated` (Douyin's status 8, "not logged in") and the hint to
log in again.

### Risk control

- Every web API call is signed (`a_bogus`, plus `x-secsdk-web-signature`
  on the paths the web SDK protects), and carries the visitor ids `ttwid`
  and `s_v_web_id`, which ATP registers once and reuses for a week.
  Douyin still scores clients: an empty answer or a page instead of data
  is `verification_required` (Douyin refuses the call to visitors or
  flagged clients). Slow down, open douyin.com in the browser, and retry
  later; log in with `--browser`, or use a residential network in mainland
  China. On a terminal ATP opens douyin.com in a browser for the check (see
  `atp guide login`). ATP keeps at least 800 ms between requests.
- Likes, follows, saves and comments also carry device traits
  (`x-tt-session-dtrait`, from a captured Chrome profile); writes sent as
  POST carry the secsdk CSRF token, and ATP waits a random 2 to 6 s
  before each write. They are untested against the live site: another client
  that reproduces them reports likes sent without device traits answered
  with HTTP 403 and the session logged out. Try them on an account you can
  afford to log in again.
- When an answer asks for a captcha (`x-vc-bdturing-parameters`) or, on a
  write, an identity check (`x-tt-verify-passport-decision`), the command
  stops with `verification_required`; on a terminal ATP opens douyin.com
  in your browser. Whether passing a check there clears ATP's next call
  is not known: if it does not, do the same thing once on douyin.com and
  retry later.
- Publishing and deleting go to creator.douyin.com like its web uploader:
  `a_bogus` with the creator page's ids, the secsdk CSRF token, and the
  ticket-guard signature its consumer SDK makes (an HMAC under a key
  agreed with Douyin's built-in server key, over the passport's clock).
  When Douyin wants an identity check (二次验证), a captcha, or answers
  with nothing, the command stops with `verification_required` and the
  upload page; ATP does not open it for you, since passing a check there
  does not clear the call ATP made. Publish once on creator.douyin.com in
  the browser (pass the check it asks for), then retry.

## References

- Posts: the aweme id, `https://www.douyin.com/video/<id>`, `/note/<id>`
  (image posts), links with `?modal_id=<id>`, `iesdouyin.com/share/...`
  pages, `https://v.douyin.com/...` short links or shared text containing
  one.
- Users: the `sec_uid` (`MS4wLjABAAAA...`), `https://www.douyin.com/user/<sec_uid>`,
  or `@douyin_id` (looked up by search, so it needs a login).
- Mixes (合集): the id or a `douyin.com/collection/<id>` link; music: the id
  or a `douyin.com/music/<id>` link; live rooms: the `web_rid`, `live:<web_rid>`
  or a `live.douyin.com/<id>` link.

## Commands

### search

`--sort general|likes|latest` plus at most one `--filter`: a time
(`day`, `week`, `half-year`), a type (`video`, `images`), a length
(`short`, `medium`, `long`) or a range (`unseen`, `following`). Without a
login Douyin usually returns nothing, reported as `verification_required`.
`search -t user QUERY` finds users. `search -t topic WORDS` lists the
hashtags the creator platform's publish form suggests for the words
(`challengesug`), each with its id and views; it needs a login and is
untested.

### hot

The hot boards (`--category all|entertainment|society|challenge`), each
word with its heat (`metrics.heat`) and views. When the signed web board is
walled, the open billboard (words and heat only) stands in for `all`.

### feed

`--type recommend` (default), `following` or `friends`; the last two need a
login.

### following

`following [USER]` lists whom a user follows, latest follow first. Rows
carry `extra.mutual` (you follow each other), `extra.remark` (the name you
gave them) and `extra.special` (a special follow, 特别关注) where Douyin
tells them. On your own list:

- `--sort recent|oldest|smart`: latest follow first (the default),
  earliest first, or Douyin's own mix (综合排序), as the following page
  offers them.
- `--search WORDS` finds the accounts you follow by name or Douyin id
  through the page's search box (Douyin's suggestions, at most 100).
- `--mutual` lists your mutual follows (互相关注), from the relations the
  share panel loads.

`--sort`, `--search` and `--mutual` go one at a time.

### remove-follower, block, unblock

One user per call. Douyin's web has no list of the accounts you block,
so there is no `blocked`.

### approve-follower

`approve-follower USER…` lets people who asked to follow your private
account (关注请求) follow it, one per call. Needs ATP's own login.

### likes

`likes [USER]`: posts a user liked (public likes only); yours when USER is
omitted.

### collections, favorites

Your own account only. `collections` lists your folders of saved posts
(收藏夹); `favorites [--folder ID]` your saved posts, all or those of one
folder.

### folder-create, folder-edit, folder-delete

`folder-create NAME [--private]` and `folder-edit FOLDER [--name NAME]
[--private | --public]` work like the 收藏夹 tab: names of 15 characters
at most; folders have no description or cover. `folder-edit` without
`--private` / `--public` keeps the folder's privacy (read from
`collections`). `folder-delete` takes one folder per call; what becomes
of the posts in it is not known. FOLDER is an id or `#N` after
`collections`. Needs ATP's own login.

### folder-add, folder-remove, folder-move

`folder-add FOLDER POST…` puts posts into a folder and `folder-remove`
takes them out, 100 per call. `folder-move FROM TO POST…` adds to TO,
then removes from FROM (`--copy` keeps them in FROM). Whether a post must
be saved first, whether `folder-remove` also unsaves it, and whether
adding to one folder takes a post out of another (the call is named
`collects/video/move`) are not known. Needs ATP's own login.

### saved-mixes, saved-dramas, saved-music, save, save-drama, save-music

The other tabs of 收藏: `saved-mixes` lists the mixes (合集) you saved,
`saved-dramas` the short dramas (短剧) and `saved-music` the music.
`save MIX…`, `save-drama DRAMA…` and `save-music MUSIC…` save them and
with `--undo` unsave them; several mixes or dramas unsaved together go 50
per call (no write key needed), the rest one by one. MIX is an id, a
douyin.com/collection/… link or `#N`; DRAMA an id or `#N` after
`saved-dramas`; MUSIC an id, a douyin.com/music/… link or `#N` after
`saved-music`.

### history, history-clear

`history` is what you watched on this account. `history-clear` erases
all of it after asking (`-y` in scripts); the web offers no way to remove
single posts, so there is no `history-delete`.

### watch-later, later-add, later-remove, later-clear

Your watch-later list (稍后再看): `watch-later` lists it, latest added
first; `later-add POST…` adds posts (one call each), `later-remove POST…`
takes them out (50 per call), `later-clear` empties it after asking (`-y`
in scripts) and `later-clear --watched` drops only what you watched. The
result of `later-clear` says how many posts went.

### notifications

The notice center (互动消息) of your account:
`--type all|follower|mention|comment|like|danmaku`.

### like, unlike, favorite, unfavorite

Like posts or save them to your favorites (收藏), or undo either. Several
posts go one by one, except that `unlike` with several takes them back 50
per call (`cancel/item/digg/multi`, as the likes tab does) and
`unfavorite` with several unsaves them 50 per call
(`aweme/cancel/collect/multi`, as the 收藏 tab does); posts Douyin names
as not done there fail. Saving into a folder (`--folder`) is not
offered: save the post, then `folder-add`. Needs ATP's own login (see
Logging in), except `unfavorite` with several posts.

### comment, delete-comment

`comment POST TEXT` comments on a post; `--reply-to COMMENT` answers one of
its comments (the id `comments` lists). Comments are text only.
`delete-comment POST COMMENT` asks first (`-y` in scripts). Both need
ATP's own login; comments are signed with its key.

### follow, unfollow

Follow or unfollow users (a `sec_uid`, a profile link or `@douyin_id`),
one by one. Needs ATP's own login.

### live

`live ROOM`: title, anchor, audience and the FLV / HLS stream addresses in
`extra.streams`.

### listen

`listen ROOM …` follows live rooms (a `web_rid`, `live:<web_rid>` or a
live.douyin.com link) over Douyin's webcast websocket, without a login.
Each event is a post whose `kind` is `message` (danmaku), `gift` (name ×
count, `extra.count`), `like` (`extra.likes`, `extra.likes_total`),
`enter` (someone joined, `extra.audience`), `follow`, `share`, `stats`
(the audience count, `extra.viewers`) or `offline` (the live ended, which
ends the stream); `extra.chat` is `live:<web_rid>`. A room that is not
live is `not_found`.

### login-sms

Logs in with a phone number and a texted code; see `atp guide douyin
login`.

### mix, music, related

`mix MIX` and `music MUSIC` list the posts of a mix or a music; `related
POST` the posts Douyin recommends next to one.

### post

Publishes through the creator platform (creator.douyin.com) the way its
web uploader does. The files are uploaded first (videos in parts, with
progress; a video upload that stopped continues when the same command
runs again within two hours), then the work is created. Before anything
is uploaded ATP prints what it is about to publish (format, title,
visibility, the AI label, place, activity, files and sizes) and asks;
scripts pass `-y`.
The answer is the new work's id (`post` → `id`), which `delete` and
`read` take.

One of three formats per post:

- **Video**: `--video FILE`, an optional `--title` (30 characters at
  most) and TEXT (the description, 1000 at most). Without `--cover`
  Douyin's first frame is the cover; `--cover IMAGE` uploads one.
  A video has two covers on Douyin: the cover (竖封面, shown at 3:4) and
  a landscape one (横封面, 4:3); `--cover-horizontal IMAGE` uploads the
  second. Given alone, it is the main cover too, as the web page makes
  it.
- **Image post (图文)**: `-i IMAGE` (repeatable, in order; the first is
  the cover), `--title` (20 at most), TEXT, and `--music ID` for
  background music (ids from `atp douyin musics`).
- **Article (文章)**: `--article FILE`, a Markdown or plain-text file sent
  as the body (100 characters at least; pictures inside it are not
  uploaded, so `![...](...)` is refused), `--title` (required, 30 at
  most), `--cover IMAGE` (required), TEXT as the summary (30 at most),
  and `--music ID`.

Options for all three:

- `--topic NAME` (or `--tag`, repeatable) adds a hashtag after the text.
  Videos and image posts take any name, as typed in the editor; an
  article's topics must be ones Douyin suggests for that exact name.
- `--visibility public|friends|private` (default public; `private` is
  仅自己可见). Posts in a collection must be public.
- `--ai` sets Douyin's 「内容由AI生成」 declaration (自主声明), which China's
  rules on labelling AI-generated content require for AI-made or
  AI-narrated work: the item carries `declare.user_declare_info` =
  `{"choose_value":"aigc"}`. Videos and image posts only; the article
  form has no declaration.
- `--schedule WHEN` publishes later, 2 hours to 14 days ahead (`+3h`,
  `2026-10-20 20:00` in local time, or an RFC 3339 time).
- `--collection NAME|ID` adds the post to one of your collections (合集)
  as its next episode (`atp douyin mixes` lists them).

Videos and image posts also take (the article form has neither):

- `--location ID` tags the post with a place (位置), by an id `atp
  douyin poi KEYWORD` printed (or `#N` right after it). ATP keeps the
  places of a week's searches, since the location box finds places by
  name only; an id it does not know is searched for as words, and a
  place's exact name works too. The item's `anchor` carries the place
  as the box picks it: `poi_id`, `poi_name` and `anchor_content`, the
  search result's `anchor_post_ext.anchor_content` with its
  `is_commerce_intention`. Places the box does not attach at once are
  refused: promoted housing (a lead form) and those behind a notice to
  confirm.
- `--activity ID` joins an official activity (官方活动), by an id `atp
  douyin activities` printed (or `#N` right after it). It must be in
  that list, not ended, and have a hashtag. The post joins through the
  activity's hashtag, as the page's activity picker inserts it: a
  `--topic` of that name, or the `#name` written in TEXT, becomes it,
  else it goes after the text and topics. It is sent as a hashtag with
  the activity's id (`text_extra`, `challenges`, `hashtag_source`
  `recommend`), and the id goes in `activity`. The summary names the
  activity, the hashtag and its end.

Videos also take `--chapters FILE` (视频章节): a file of one `0:00 Title`
line per chapter (`-` reads standard input), shown on the video's
progress bar as the publish page's chapter editor saves them. The
editor's rules, checked before anything is uploaded:

- videos of a minute or more (MP4 and MOV files are measured first);
- 1 to 20 chapters, at least 5 s apart, none past the end of the video
  (the last one at least 5 s before it);
- titles of 12 at most, counted as the editor counts them: Latin
  letters, digits, `+` and `-` count half, anything else one (`第一章
  开场` is 6, `Setup and build` 9);
- the first chapter may start after 0:00 (the player shows the part
  before it untitled).

The chapters go out with the "progress bar" setting on (`chapter_type`
1, the editor's default; ATP has no option to turn it off), without a
summary (全文总结) or chapter introductions, and are reviewed with the
video.

Covers go as they are: the web page crops a picked image to 3:4 or 4:3
in its cover editor, ATP does not. The summary notes an image far from
its shape.

```
atp douyin post --video ep01.mp4 --cover ep01.jpg --title '『操作系统 Ep.01』绪论' --ai --visibility private
atp douyin post --video ep01.mp4 --cover v.jpg --cover-horizontal h.jpg --title 绪论
atp douyin post -i a.jpg -i b.jpg -i c.jpg --title 测试 '正文' --topic 摄影
atp douyin post --article notes.md --title 读书笔记 --cover head.jpg '一句话摘要'
atp douyin post --video talk.mp4 --title 讲座 --chapters chapters.txt
```

where chapters.txt holds:

```
0:00 开场
1:05 环境搭建
4:30 Demo
12:10 问答
```

A place and an activity, found first:

```
atp douyin poi 三里屯                  # ids, names and addresses
atp douyin post --video v.mp4 --title T --location <poi_id>
atp douyin activities                  # ids, dates and hashtags
atp douyin post --video v.mp4 --title T --activity <id>
atp douyin post -i a.jpg -i b.jpg --title 秋天 '银杏' --location '#3'
```

### edit

Changes one of your published videos or image posts the way the creator
platform's edit page (作品管理 → 修改作品) does. It loads the work first,
prints the change (each field as old → new) and asks (`-y` in scripts).

- TEXT replaces the description; the hashtags stay unless `--topic` is
  given.
- `--title` replaces the title (30 characters for videos, 20 for image
  posts).
- `--topic NAME` (repeatable) replaces the hashtags, which go after the
  text as they do on `post`; `--no-topics` removes them. Without TEXT the
  description stays and only its hashtags change.
- `--cover IMAGE` uploads a new cover (竖封面, 3:4) and
  `--cover-horizontal IMAGE` a new landscape one (横封面, 4:3); each
  leaves the other as it is. A video whose cover is landscape (from
  before videos had two) keeps that one as its 4:3 cover beside a new
  `--cover`, as the edit page does. As on `post`, the images go
  uncropped, with a note when one is far from its shape.
- `--visibility public|friends|private` goes through the manage page's
  permission setting; it does not count as an edit.
- `--chapters FILE` replaces a video's chapters, through the manage
  page's chapter editor (作品管理 → 编辑章节, `chapter/upsert`), under the
  rules of `post --chapters` checked against the video's length;
  `--no-chapters` removes them (`chapter/delete/`). The new chapters are
  reviewed before they show (确认成功，已提交审核); the work's summary
  and progress-bar setting stay. Douyin refuses while chapters sent
  earlier are still in review, and when it locks a video's chapters
  (`permission_denied`).
- `--video FILE` replaces the video (更换视频): the file is uploaded as
  on `post` and the edit page's form carries its id. Douyin offers this
  only on some works and says why not (`permission_denied` with its
  reason): at most 3 replacements per work, none while the work is in
  DOU+ delivery or in review for a promotion task or activity. Before
  asking, ATP lists what follows, as the page does: the new video is
  reviewed again and replaces the old one once it passes (likes,
  comments and other counts stay); it must not differ much from the old
  one; a music set on the old video gives way to the new file's sound;
  bullet comments (弹幕) are cleared within 24 hours, chapters must be
  confirmed again and components (hotspots, labels) are reviewed again
  where Douyin says so. The cover stays unless `--cover` gives one.
  `--video` and `--chapters` go in separate edits: set the chapters
  again once the new video has passed review. Douyin videos have one
  part, so there is no `--part`.
- Douyin allows each work 5 edits (每个作品最多修改5次), so batch the
  changes. Every edit that changes the text, title, hashtags, covers or
  video uses one (the edit page's `update/desc/`); chapters and
  visibility go through the manage page and use none. The summary shows
  the edits left (from the edit page's `update/desc/valid/v2`:
  `edited_times` of `max_edit_times`) and whether this change uses one,
  and warns before the last one. With none left, or when Douyin says the
  work can no longer change, the edit fails with `permission_denied`
  before anything is uploaded. Changed works may be reviewed again.
- `--dry-run` checks first without using an edit: it loads what an edit
  loads (`item/info`, `update/desc/valid/v2`, the chapters), prints the
  summary with the edits left, whether the work can be edited, whether
  its video can be replaced and whether its chapters are locked or in
  review, and a `refused` row for each thing ATP or Douyin would refuse;
  then the text as `item/info` gives it (`desc`, `item_title`,
  `caption`, each `text_extra` entry, the @-mentions loaded from them,
  the hashtag ids of `activity_list`) and the exact forms it would send
  (`update/desc/`, `chapter/upsert` or `chapter/delete/`,
  `aweme/update/`), uploads shown as `<uri of FILE after upload>`;
  `update/desc/` only when its text can be made. It uploads and sends
  nothing. Once the work loads it always prints, and it ends with status
  0 even when it reports a refusal (its message then says the edit would
  be refused), as it only reports; the same edit without `--dry-run`
  fails with that refusal's error. Without other options it shows the
  work's state.
- The text keeps what the edit page keeps. Its editor knows five
  `text_extra` types: an @-mention (0), a hashtag (1), an image post's
  title and the `。` after it (7, 8) and an activity's hashtag (9, sent
  back as a hashtag that joins the activity). A mention (with its
  `user_id`, and `mentions`) goes back as loaded while the text stays;
  with a new TEXT or `--topic`, each mention whose `@name` is still in
  the text is kept at its new place, and the others go as plain text, as
  when one is deleted in the page's editor (the summary names them). A
  work joins an activity through a hashtag of the activity's
  (`activity_list[].challenge_ids` of `item/info`); the form's
  `activity` lists those hashtags, so the activity stays while its
  hashtag does. The summary shows mentions and activities as old → new.
- Other `text_extra` types are links the editor does not know: a search
  word (搜索词, type 5, douyin.com's `Search`), which Douyin adds itself:
  a word of the text linked to a search for it, with `search_text` and
  `search_query_id` (`item/info` can hold one that the public video
  page does not show); a group chat (type 16, `IMGroup`); and others.
  The edit page's editor leaves their words in the text as plain text
  and sends only hashtags and @-mentions back, so an edit that sends the
  text (on a video every edit through `update/desc/`, covers too; on an
  image post one that changes its words) drops them, the text changed
  or not. The summary names each one that goes; Douyin may link the
  words again. (When such a link comes after the last hashtag or
  mention, the page's editor also loses the words before it; ATP keeps
  the text whole.) An edit that sends a text whose @-mention is not
  where Douyin says is refused; its chapters and visibility can still
  change. Articles are refused.

```
atp douyin edit 7697611474302225714 --topic 操作系统 --topic 计算机
atp douyin edit https://www.douyin.com/video/7697611474302225714 --title '『操作系统 Ep.02』硬件视角'
atp douyin edit '#1' --visibility private
atp douyin edit 7697611474302225714 --dry-run
atp douyin edit 7697611474302225714 --cover v.jpg --chapters chapters.txt --dry-run
atp douyin edit 7697611474302225714 --chapters chapters.txt
atp douyin edit 7697611474302225714 --no-chapters
atp douyin edit 7697611474302225714 --video ep02-fixed.mp4
atp douyin edit 7698707923228265754 --cover v.jpg --cover-horizontal h.jpg
```

### delete

Deletes one of your works through the creator platform (the id, a link,
or `#N`), after asking (`-y` in scripts).

### mixes

Your collections (合集) on the creator platform, with their episode
counts, as the publish form lists them (待发布 and 已发布; not those in
review or shown to you only); `post --collection` takes a name or an id
from here. `mix-create` and the commands below make and change them.

### mix-create, mix-edit, mix-delete, mix-add, mix-remove, mix-sort

Your collections (合集) as the creator platform's 合集管理 pages keep
them, through those pages' own calls. Each loads what it changes, prints
the change and asks (`-y` in scripts). MIX is a collection's id, its
douyin.com/collection/… link, its exact title (one in review or shown
to you only is found too) or `#N` after `mixes`; WORK is one of your
works: its aweme id, a link, or `#N` after `works`.

- `mix-create TITLE [WORK…]` creates one with the works it starts with,
  as its episodes in the order given, in one call (`mix/create/` with
  `item_ids`), as the page's 创建合集 form sends its 添加作品 list. The
  title takes 2 to 20 characters and no emoji (合集标题不允许输入emoji);
  `--desc` 5 to 200 (line breaks go, as in the page's box). `--cover`
  is a JPG or PNG of 5 MB at most and 100×100 at least; the page crops
  it square (1080×1080 shows best), ATP sends it uncropped and says when
  it is far from square. Without `--cover` the collection takes its last
  work's cover, as the page fills it in.
- `mix-edit MIX` changes the title (`--title`), description (`--desc`)
  or cover (`--cover`) through `mix/update/`, which carries all three:
  what is not given stays. Douyin reviews the change before it shows.
- `mix-delete MIX` deletes the collection (`mix/destroy/`); its works
  stay on your account (删除合集时不会删除合集里的视频).
- `mix-add MIX WORK…` adds works after the last episode, in the order
  given; `mix-remove MIX WORK…` takes works out (they stay on your
  account); `mix-sort MIX WORK…` puts the works given first, in that
  order, and the others after them as they were (the page's dragging,
  置顶 and 修改作品集数). All three load the collection's works
  (`mix/item/list/`) and send the whole new list in one call
  (`mix/item/editlist/`: `item_ids` in order, with `add_item_ids` and
  `del_item_ids` against the old list), as the page's 保存 does.
- None of these uses one of a work's 5 edits (每个作品最多修改5次): they
  are the 合集 pages' calls, while the edits are counted on the edit page
  (`update/desc/`), whose form has no 合集 field (the publish form hides
  it when editing). A published work joins a collection without an
  edit, and the summary says so.
- Douyin's rules, as the page holds works to them: a work is in one
  collection at most (已加入其它合集: take it out there first); only
  public works join (private and friends-only ones do not:
  `edit WORK --visibility public` first, which uses no edit); works in
  review or not passed, live replays, daily posts (日常), others'
  co-created works and product image posts (商品图文) do not; image
  posts and videos both do. One change adds 100 works at most. A
  collection of fewer than 2 works shows to you only (others see it from
  2), and one with none is 待发布. ATP checks visibility and another
  collection itself (from `item/info`, as `edit` loads a work), Douyin
  the rest.
- Douyin reviews a collection's title, cover and description; while
  one is in review (审核中) the page lets nothing in it change, and ATP
  refuses too (`permission_denied`). Paid collections (付费合集), those
  for members (会员合集), short dramas and live replays are not offered.

Putting three published 操作系统 episodes into a new collection, in
episode order, in one step (uses no edits):

```
atp douyin works -n 20                 # find the episodes' ids
atp douyin mix-create 操作系统 7697000000000000001 7697000000000000002 7697000000000000003
```

Or create it first and add them after; if Douyin has the new collection
in review by then, `mix-add` is refused until the review is over, which
the step above avoids:

```
atp douyin mix-create 操作系统 --cover os.jpg --desc '从硬件到进程的操作系统课'
atp douyin mix-add 操作系统 7697000000000000001 7697000000000000002 7697000000000000003
atp douyin mix-sort 操作系统 7697000000000000003     # move the third to episode 1
```

Untested: built from the 合集 pages' bundles, not run against an account.

### musics

The music library of image posts and articles: without an argument its
categories (`TYPE:ID`), with one the songs of that category, each with
its id and length in seconds. Searching the library is not offered (the
page searches through a separately signed service).

### insights

`insights POST` gives the numbers of one of your works as the creator
platform's works list (`creator/item/list`, read by its manage and data
pages) has them, all untested:

- plays (`views`), `likes`, `comments`, `shares`, saves (`favorites`),
  `danmaku`, `new_followers` (吸粉量) and `cover_impressions` (封面展示);
- `cover_ctr` (封面点击率), `completion_rate` (完播率),
  `five_second_completion_rate`, `two_second_bounce_rate` (2 秒跳出率),
  `avg_watch_seconds` (人均播放时长) and `avg_watch_ratio` (平均播放占比);
- for image posts `text_expand_rate` (文案展开率) and `avg_images_viewed`;
  for articles `detail_open_rate` (详情页进入率) and
  `read_completion_rate`.

Rates are fractions from 0 to 1 (a value above 1 is read as a
percentage, as the page reads it).
Numbers ATP does not know, such as the split between followers and other
viewers, keep Douyin's own names and values. `extra` holds the work's
`type`, `review` state and `visibility` (see `works`). The work is looked
up among your latest 1000 works; the numbers are its totals so far
(`totals_period: lifetime`), without trends or breakdowns, and `--days`
is not used. `insights` without POST (the whole account, 数据中心) is not
offered. Any login works, `--browser` and `--cookie` sessions included.

### works

`works` lists your works, latest first, with the numbers of `insights`:
plays, likes, comments, shares and saves in `metrics` (with `danmaku`,
`new_followers` and `cover_impressions`), the rates and averages in
`extra`, and

- `extra.review`: `reviewing`, `passed`, `self_only` (passed, shown to
  you alone), `rejected`, `advice` or `friends_only`;
- `extra.visibility`: `public`, `friends` or `private`.

These names come from the enums of the creator platform's item service
in its page bundles, which do not explain `advice` and `friends_only`;
an unknown code shows as `code N`. `-f csv` gives one row per work (run
it from cron for a series over time); `--raw` shows a row as Douyin sent
it. Untested.

### drafts

`drafts` lists your drafts (草稿) on the creator platform, with their
text and times; they have no link. Saving a post as a draft is not
offered: on the web only articles are saved to the creator platform.
Its draft calls (`aweme/draft` with `req_type` 0 save, 1 open one by
`creation_id`, 2 discard, 3 the latest, 4 one handed over from another
page) are made for the article form, which saves every half second and
on 暂存离开 (save and leave), and refuse any other form. On the video
and image-post forms 暂存离开 sends nothing: the page leaves, and what it
keeps stays in the browser. Untested.

### poi

`poi KEYWORD` finds locations (POI, 位置) as the publish form's location
box does on its domestic tab (国内): name, address (`description`),
`extra.city` and the id, which `post --location` takes. ATP keeps the
places found for a week (in its cache) for that. Untested.

### hotspots

`hotspots` lists the hot spots (热点) the publish form offers to link a
post to (关联热点): those Douyin recommends (`extra.recommended`), then
all, each with its heat (`metrics.heat`); `hotspots WORDS` searches them.
Linking a post to one is not offered. Untested.

### activities

`activities` lists the official activities (官方活动) the publish form's
activity picker shows your account: name, dates and hashtags
(`description`), `extra.heat`, `extra.topics` and their ids
(`extra.topic_ids`), `extra.end_time`, and Douyin's codes for the kind,
reward and category. `post --activity ID` joins one, through its
hashtag with its id (see post). Without it, a `--topic` of an
activity's hashtag stays a typed hashtag without the id (the page's
editor would turn it into the activity's). Untested.

### download

Videos at the highest bitrate without watermark, every image of an image
post (with live-photo clips) and its music; `--audio-only` saves the music.

## Limits

- Publishing and deleting are built from the contract of the creator
  platform's web client (its page bundles), and have not been run against
  a real account yet.
- Chapters and video replacement are built the same way and are
  untested too. Not confirmed: whether `item/info` carries a work's
  chapters, summary, progress-bar setting and review state as the manage
  page's work list does (when it does not, `edit --chapters` cannot show
  the old chapters, resets the summary and turns the progress bar on);
  whether `update/desc/valid/v2` with `action=init` already holds the
  `media_fields` the page checks again before sending a new video; and
  how Douyin picks the cover of a replaced video when the old one is
  sent back. `read` gives a video's chapters (`extra.chapters`, with
  `chapters_auto` for smart chapters Douyin made itself) from the
  aweme's `chapter_list`, as the web player reads them.
- Not offered with chapters: their introductions (章节简介) and the
  summary (全文总结), smart chapters (智能章节) and turning off the
  chapter progress bar.
- Formats the web uploader also offers that ATP does not: panoramic
  video (全景视频), which needs the projection settings and a server-side
  transcode change (`vr/transcode/change`) ATP does not reproduce; short
  drama (短剧), which needs a playlet qualification and its own series.
  Daily posts (日常) and text-only posts are made in the app; the web
  uploader has neither.
- Not offered when publishing: mentions (@), anchors other than a place
  (goods, mini programs ...), hot spots (`hotspots` lists them), saving
  as a draft (the web saves articles only, see drafts), co-creators,
  paid content, choosing a video's music (Douyin picks it), editing or
  cropping a cover in the page's editor, and pictures inside articles.
- Collections (`mix-create` … `mix-sort`) are built from the 合集管理
  pages' bundles and untested too. Not confirmed: whether `item/info`
  names the collection a work is in (`mix_info`; when it does not,
  Douyin's own refusal shows), whether `mix/create/` answers with the new
  collection's id (ATP looks for `mix_id`), whether a new collection goes
  into review at once, and whether Douyin takes a work it would refuse
  silently: `mix MIX` shows the collection's posts once it is public.
- `--location` and `--activity` are built from the publish form's
  bundles and untested too. Not confirmed: whether the location box
  answers a search for a place's id (ATP keeps searched places for
  that reason), and whether activity hashtag ids come as numbers (ATP
  sends numbers, as it does for an edit).
- Writes are untested against the live site (see Risk control), and so
  are the following orders, `--search` and `--mutual` (built from the web
  client's page bundles): how `oldest` and `smart` page on, and whether
  the share panel's relations hold all your mutual follows, are not
  confirmed.
- The web offers no follow groups, list of blocked accounts, way to set
  remarks or special follows, or folder covers, descriptions and order,
  so ATP has none of them either.
- The creator platform's reads (`insights`, `works`, `drafts`, `poi`,
  `hotspots`, `activities`, `search -t topic`) are built from its page
  bundles and another client's calls, and have not been run against an
  account. Not confirmed: the field groups `works` asks for
  (`default,metrics,review,visibility`: `default`, which should carry the
  title and time, is the data page's, the others another client's);
  where a row keeps its review state and visibility, and which codes
  they use; whether the drafts list answers a plain request (the page's
  security SDK signs that path with the write key, ATP does not) and
  whether it pages; and how the location box pages on. Account-wide
  analytics (数据中心) are not offered.
