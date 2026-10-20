ZSXQ (知识星球, wx.zsxq.com) is where creators run paid communities
(星球, "groups" here): members post topics (主题), ask the owner questions
(问答), share files and articles, and the best topics are marked as
digests (精华). ATP reads the JSON API the site's web app calls: the
groups you joined, their topics with pictures, files, articles and Q&A
answers, their digests, the feed of all your groups, comments, search,
members and the files of a topic. With the login it also likes, comments
and posts text. `知识星球` works as the platform's name too.

Almost everything on the site is for the members of a group, and so is
almost everything here: it needs a login, and a group's topics need its
membership. Without an account only a group's public preview and a
member's public profile answer.

The login's commands (everything but `group`, `topics` without a login
and `user`) were written from the web app's own code and have not been
tried with a real account: their answers were not seen, and the writes
(`like`, `comment`, `post`) were never sent. Report what breaks.

## Logging in

The site logs in with WeChat (a QR code on wx.zsxq.com, or the app). ATP
offers no QR login of its own: the WeChat step runs on WeChat's pages,
and the site's own step sends an encrypted body ATP does not reproduce.
A login is the site's cookies of `.zsxq.com` (`zsxq_access_token` among
them; they are httpOnly, set for wx., api. and .zsxq.com).

- `atp zsxq login --browser` reads the cookies of a browser logged in at
  wx.zsxq.com. Log in there first (scan the QR code with WeChat).
- `atp zsxq login --cookie '…'` takes a `Cookie` header copied from the
  browser's developer tools: any request of the logged-in web app to
  api.zsxq.com (`-` reads it from stdin).
- `ATP_ZSXQ_COOKIE='…'` passes the cookies for one run; they are never
  saved.

`login` and `status` ask for the account (`/v3/users/self`) and fail with
`not_authenticated` when the site sees no login.

### Without an account

`group` (a group's public preview: name, description, owner, members,
topics, the invite link), `topics` (the few topics that preview shows)
and `user` (a member's public profile).

### With an account

`whoami`, `status`, `groups` (also `collections`), `feed`, `hot`,
`search`, `read`, `comments`, `user-posts`, `download`, `group-search`,
`topics`, `like`, `unlike`, `comment` and `post`. A group's topics,
comments, search and files need its membership; a group you are not
in, or no longer, answers `permission_denied` with what to do.

### How long it lasts

How long the site keeps a login is not published. When it ends (or the
site ends it because you logged in elsewhere), commands fail with
`not_authenticated` and the hint to log in again.

### Risk control

- ATP keeps at least 3 s between requests and waits 2 to 5 s before each
  write. A read that fails on the network or with a server error is
  tried up to twice more; writes are never repeated.
- Every request carries the headers the web app signs its requests
  with: a SHA-1 of the URL, the time and a random request id, without
  any key, and a browser id ATP keeps with the session. Without them the
  site answers code 1059 (设备状态异常). A read refused that way is asked
  once more with a new signature; a refusal that stays is
  `signature_error`.
- HTTP 429 (请勿频繁操作) is `rate_limited`. Muted (禁言) or banned
  accounts and groups that forbid an action are `permission_denied`.
- Paid content stays as the site gives it: ATP never joins a group, pays
  or unlocks anything.

### Environment

`ATP_ZSXQ_COOKIE`: the session's cookies for one run.

## References

- Topics: `https://wx.zsxq.com/topic/<id>` (what lists print), a link
  with the topic in a group (`…/group/<gid>/topic/<id>`) or with
  `topic_id=`, or the number.
- Groups: `https://wx.zsxq.com/group/<gid>` (what `groups` prints), the
  public page (`https://public.zsxq.com/groups/<gid>.html`), a link with
  `group_id=`, or the number.
- Users: `https://wx.zsxq.com/footprint/<uid>` (their page), a link with
  `user_id=`, or the number.
- Comments: the id `comments` prints.
- Text pasted around a link (a share message) works; short links
  (t.zsxq.com) do not: open them and use the link they lead to.
- `#N` works after any list of topics or groups.

## Commands

### groups, collections

The groups you joined, in the app's order: name, description, owner,
members (`followers`), topics (`items`), and in `extra` the group's kind
(`type`: `pay`, `free` …), category, when it started, its latest topic,
when you joined (`joined_at`) and when your membership ends
(`expires_at`). Needs a login.

### group

`group GROUP` shows one group. A member sees what `groups` shows; anyone
else, or anyone without a login, gets the group's public preview
(`extra.preview`): the same fields without the membership, and the
group's invite link (`extra.invite_url`, t.zsxq.com).

### topics

`topics GROUP` lists a group's topics, newest first, by the app's tabs:
`--scope all` (the default, 全部), `digests` (精华), `owner` (只看星主)
or `questions` (问答). Twenty a page; the cursor is the `end_time` of the
last topic (`2026-10-20T12:00:00.000+0800`), and `--until` starts there
when no cursor is given, so `--since` and `--until` read any stretch of
a group's history without paging through the newer part. Without a login
it lists the few topics the group's public page shows (digests mostly,
their texts cut short), whatever the scope.

### hot

The digests (精华) of your groups, one group after another in the order
of `groups`: all of the first group's, then the next group's. The cursor
holds the group's place and the `end_time` inside it (`2~…`). A group
that refuses (an expired membership) is skipped with a note. For one
group: `topics GROUP --scope digests`. `--since` ends the whole listing
at the first group whose digests are all older; for a window in every
group, run `topics GROUP --scope digests --since …` per group.

### feed

The feed of all your groups (动态), newest first: the topics as they
come, each with its group (`extra.group`) and what happened
(`extra.action`). Thirty a page, paged by `end_time`.

### read

A topic: its text, author, time, pictures (the largest size the API
gives; their links are signed and expire), likes, comments, reads
(`views`), 赞赏 (`metrics.rewards`), readers (`metrics.readers`), the
hashtags (`tags`) and in `extra` its group, `digested`, `sticky`,
`liked` (by you), its files (`files`: id, name, size, downloads), the
long article it links (`article`: title and link on articles.zsxq.com)
and its video (`video`: id, length, cover; not downloaded). A topic's
`kind` is `topic` (a post, 谈话), `question` (问答), `task` (an
assignment, 作业) or `solution` (an answer to one).

A question's text is the question and, once answered, the answer under
it (`Q: … A (who): …`); `extra` holds who was asked (`questionee`),
`answered`, the answer (`answer`: author and text) and `anonymous`.
Paid or hidden parts stay as the site gives them. Mentions, hashtags and
links in texts read as `@name`, `#tag#` and `title (link)`.

### comments

A topic's comments, oldest first as the app shows them, thirty a page,
the pinned one first (`extra.sticky`). Each has its likes, the number of
replies (`reply_count`), the one it answers (`reply_to`), its pictures
(`extra.images`), 赞赏 (`extra.rewards`) and the replies the site nests
in it (`replies`; possibly not all of them). The cursor is the time of
the last comment; `--sort` is not offered.

### search

`search WORDS` searches the way the site's search box does outside a
group (`scope=all`, the default order). What it covers is the site's
choice; for one group use `group-search`.

### group-search

`group-search GROUP WORDS` searches the topics of one group, in the
site's default order, twenty a page.

### user

`user USER` shows a member's public profile, with or without a login:
name, avatar, introduction, location, followers, following and notes
(`stats.notes`).

### user-posts

`user-posts USER` lists the topics of a member's page (足迹): what they
posted in groups you can see, newest first, paged by `end_time`; their
notes (笔记) are left out. Members who hide their page answer
`permission_denied`.

### download

`download TOPIC` saves a topic's pictures and its files. Each file's
link is asked when it is fetched (the site counts that as a download,
and groups may forbid downloads), and the file keeps its own name; an
interrupted file continues from its `.part` on the next run. Videos are
not downloaded.

### like, unlike

Like a topic, or take the like back. Not tried with a real account.

### comment

`comment TOPIC TEXT` comments on a topic; `--reply-to COMMENT` answers
one of its comments (an id `comments` prints). Text only. Accounts
without a phone number may be refused (`permission_denied`). Not tried
with a real account.

### post

`post TEXT --topic GROUP` posts a text topic to a group, after asking
(the group's name is shown; `-y` in scripts). One `--topic`, the group
(link, number or `#N` after `groups`); no `--title`, pictures or files.
`--reply-to TOPIC` comments on that topic instead. Not tried with a real
account.

## Limits

- Everything but a group's public preview and public profiles needs a
  login and, for a group's content, its membership. ATP never joins,
  pays, renews or unlocks.
- No QR or phone login: log in on wx.zsxq.com and import the cookies.
- Writes are text only: no pictures, files, questions, assignments,
  editing or deleting. Nothing written has been tried.
- No notifications, private messages, notes (笔记), columns (专栏),
  check-ins (打卡) or files listed by group; videos and the long articles
  of articles.zsxq.com are linked, not read.
- Short links (t.zsxq.com) are not followed.
