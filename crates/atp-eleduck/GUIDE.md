Eleduck (电鸭社区, eleduck.com), the community for remote and part-time
work, through the JSON API its own pages call: the jobs channel (机会)
with its three kinds of job posts and its tag filters, the discussion
categories, search, posts with what they say about the job, comments with
their replies, users and their posts. With a login: upvote, save and
comment, your saved and viewed posts, your messages and your job
applications.

## Logging in

### WeChat QR code

`atp eleduck login` shows the QR code of the site's WeChat login. Scan it
with WeChat: it opens eleduck.com's WeChat sign-in page, which signs the
WeChat account in, and ATP keeps the login token the site then hands out.
The WeChat account must belong to an Eleduck account; a new one is led to
the site's sign-up instead, which ATP does not do. ATP waits up to three
minutes; when the site refuses the QR code, it has expired and `login`
fails. This login has not been tried with a real account.

### Token, browser or cookie

The site keeps its login in the `token` cookie of eleduck.com.

- `atp eleduck login --browser` reads the cookies of a browser logged in
  at eleduck.com.
- `atp eleduck login --token 'TOKEN'` takes the value of that cookie
  (developer tools, Application, Cookies, `https://eleduck.com`, `token`;
  `token=TOKEN` works too, `-` reads it from stdin).
- `atp eleduck login --cookie 'token=…'`: a `Cookie` header holding it.
- `ATP_ELEDUCK_COOKIE='token=…'` passes it for one run; it is never saved.

Phone and password logins (with the site's Aliyun captcha) and texted
codes are not offered.

### Without an account

`hot`, `jobs`, `search`, `read`, `comments`, `user`, `user-posts`,
`categories` and `tags` work without logging in.

### With an account

`whoami`, `feed`, `favorites`, `history`, `notifications`, `unread`,
`applications`, `like` / `unlike`, `favorite` / `unfavorite` and
`comment`. ATP sends the token as `Authorization: Bearer`, as the site
does, with the `X-Fingerprint-Id` the site adds when the browser's
`fingerprint_id` cookie came along (`--browser`).

### How long it lasts

The site keeps the `token` cookie for 30 days; how long the token itself
is accepted is not published. When it is no longer accepted, commands
fail with `not_authenticated` (the site's `access_not_authorized`) and the
hint to log in again.

### Risk control

- ATP keeps at least 1 s between requests and waits 2 to 5 s before each
  upvote, save or comment. The site does not publish its limits; too many
  requests are `rate_limited`.
- The site's rules for comments: no contact details (comments with them
  are deleted), and a deleted comment costs 电量 (the site's credits) and
  experience. Commenting needs a non-negative 电量 balance.
- ATP knows of no human check on the API it uses; the site's Aliyun
  captcha guards the phone logins only, which ATP does not offer.

### Environment

`ATP_ELEDUCK_COOKIE`: the session's cookies for one run.

## References

- Posts: `https://eleduck.com/posts/<id>` (what lists print; a `#…` part
  is ignored), job pages `https://eleduck.com/tposts/<id>`, `posts/<id>`,
  or the id (`GzfAwN`).
- Comments: their id, as `comments` lists it (or a link ending in
  `#comment-<id>`).
- Users: `https://eleduck.com/users/<id>`, `users/<id>`, `@<id>` or the id
  (`n0h5Ge`).
- Categories: `https://eleduck.com/categories/<id>` or the number
  (`categories` lists them).
- `#N` works after any list.

## Commands

### hot

`--category` picks the list, as the site's pages show it (25 posts a
page, pinned posts first):

- `jobs` (the default): the jobs channel, every job post;
- `community` (社区帖子招聘, posts members write), `matched` (简历智能匹配,
  openings applied to with a résumé on the site), `featured` (精选职位推荐,
  openings the site's talent service recommends): one kind of job post;
- `all`: the home page, every category;
- `talk` (职场讨论), `ai` (AI), `abroad` (海外), `stories` (项目故事),
  `freelance` (自由职业), `feedback` (曝光/反馈), `official` (电鸭官方): one
  discussion category.

`jobs` narrows the jobs channel further.

### jobs

`jobs` lists the jobs channel with its filters, each one tag of a tag
group: `--type` (工作方式: `remote`, `onsite`, `part-time`,
`offline-part-time`, `hybrid`, or the tag's name), `--city` (城市),
`--role` (职业), `--hire` (招聘类型) and `--field` (领域). Each takes the
tag's name as the site shows it (北京, 开发, 企业直招 …) or its id; `tags`
lists them all. Several filters narrow together. `--kind community|
matched|featured` keeps one kind of job post, `--new` sorts the newest
first instead of the site's own order.

    atp eleduck jobs --type remote --role 开发 --new -n 50
    atp eleduck jobs --city 上海 --kind community

### search

`search WORDS` finds posts of every category. `--sort new` puts the
newest first, `--sort hot` the most popular; the default is Eleduck's
relevance. `--filter jobs` keeps job posts only (只看招聘); the site fails
on `--sort hot` with it, so ATP refuses that pair. There is no user or
topic search.

### read

A post: its title, its text (the Markdown it was written in, else its
HTML as text), category, tags, views, upvotes (`likes`), downvotes,
comments and saves, and for a job post the job fields (see Job posts).
Contact details some job posts carry for applicants (`contact_info`) are
never shown, not even with `--raw`.

### comments

The comments of a post, 20 a page: `--sort hot` (the default), `new` or
`old`. Replies come nested in their comment (`replies`, with `reply_to`
naming whom they answer); there is no separate `replies`.

### feed

The posts of the categories you follow (needs a login).

### user, user-posts

A user's profile (bio, level, 电量 and experience, counts of posts,
comments and upvotes received, tags) and their posts, 25 a page.

### favorites, history

Your saved posts (收藏, one list without folders; the time saved in
`extra.saved_at`) and the posts you viewed in the last three months (the
time in `extra.viewed_at`); both need a login.

### notifications, unread

Your messages by the site's tabs, `--type replies` (留言与回复, the
default), `credits` (电量提示), `views` (谁看过我), `system` (社区通知) or
`jobs` (招聘通知), with the kind read from the site's (`comment`,
`reply`, `like`, `favorite`, `view`, `application`, else `system`), the
post or opening it is about and a comment's text. `unread` counts the
unread messages per tab and in all. Whether listing marks messages read is
not known.

### like, unlike

Upvote a post (点赞) or take the upvote back.

### favorite, unfavorite

Save a post to your 收藏 or take it out; there are no folders.

### comment

`comment POST TEXT` comments on a post, `--reply-to COMMENT` answers one
of its comments. The text is sent as the Markdown the site's editor
writes, with a plain HTML rendering beside it; pictures are not
supported.

### categories, tags

`categories` lists the site's categories (版块) with their code, kind
(`work` for job categories, `talk` for discussions), followers, the 电量
a new post costs and their tag groups. `tags [CATEGORY]` lists the tags
of a category (the jobs channel's by default) with their group and the
`jobs` option that takes them.

### applications

Your job applications to 简历智能匹配 openings (needs a login): the job
post, when you applied, the state (`extra.status`: `pending`,
`contact_pending`, `rejected`, `withdrawn`, `qualified`, `expired`, and
the site's own words in `extra.status_label`) and the company. `--status
processing|rejected|withdrawn|qualified|expired` keeps one state, as the
site's tabs do (`processing` is waiting to be read or contacted).

## Job posts

Job posts (`kind: job`) carry the job keys every job platform uses in
`extra`, where the post tells them:

- `remote` and `job_type` from the 工作方式 tag: 全职远程 (remote,
  `full-time`), 全职坐班 (on site, `full-time`), 线上兼职 (remote,
  `part-time`), 线下兼职 (on site, `part-time`); 混合办公 (hybrid) sets
  neither. A 外包零活 hiring tag makes `job_type` `freelance`. Posts
  without a 工作方式 tag (精选职位推荐) are read from the words of their
  title (远程, 全职, 兼职, 实习).
- `city` from the 城市 tag (海外 for abroad; 其他 is left out).
- `salary`, `salary_min`, `salary_max` (per `salary_period`),
  `salary_period` (`hour`, `day`, `month` or `year`), `salary_currency`
  and `salary_months` from the title, where it names the pay: `25K-35K`,
  `$3000-$4000`, `年薪¥95W-150W`, `17-20W/年`, `15-25K·14薪`, `$50/hr`.
  Only amounts with a unit or a currency in a range, or a currency or a
  word for pay (年薪, 月薪, 底薪, 最高 …) next to them, count; pay named only
  in the text is not read. Amounts from 100,000 up without a period (or
  `·N薪`) are taken as yearly pay; weekly pay keeps only `salary`.
- Eleduck's own: `hire` (招聘类型: 企业直招, 外包零活, 员工内推, 组队合伙,
  猎头中介), `role` (职业), `fields` (领域), and `closed` once the post is
  marked 已结束.

Eleduck posts have no company, experience, degree or deadline fields.

## Limits

- No posting, editing or deleting posts: a new post costs 电量 in most job
  categories.
- No revealing a job post's contact details (查看联系方式), which costs 电量,
  and no applying to or withdrawing from openings; `applications` only
  lists them. Applications to the talent service's openings
  (专人服务岗位) are not listed.
- No deleting or editing comments, no downvotes, no upvoting comments, no
  following categories or cities.
- No user search, and no phone or password login.
