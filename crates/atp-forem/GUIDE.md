Forem communities through their REST API (version 1): DEV (dev.to) by
default, or any other Forem community (such as community.codenewbie.org)
named by `FOREM_URL` or at login. Reading needs no account; publishing
and changing articles take an API key you make. `devto` is a shorter
name for the command.

## Logging in

Reading needs no account: `hot`, `feed`, `read`, `comments`, `replies`,
`user`, `user-posts`, `tag` and `tags` work without logging in.

For `post`, `edit`, `drafts` and `whoami`, make an API key: on dev.to
open Settings → Extensions → "DEV Community API Keys" (on another
community, the same page under its Settings), give the key a
description and generate it. Then run `atp forem login --token
API_KEY` for dev.to (or the community in `FOREM_URL`), or `atp forem
login --token 'INSTANCE|API_KEY'` for another one (`INSTANCE` is its
address, such as `community.codenewbie.org`). ATP checks the key by
reading your profile and keeps the key and the community in the
session file. `FOREM_API_KEY` supplies a key for one run instead, and
`FOREM_URL` picks the community for one run.

A key works until you revoke it on the same settings page; a revoked
or wrong key gives `not_authenticated`. The key acts as your account
(writing articles, reading your drafts) and is kept on your machine
only.

Forem does not publish its limits; it answers too many requests with
HTTP 429 and how long to wait (`Retry-After`). ATP waits out waits of
up to 30 seconds and sends the request again (twice at most), and gives
`rate_limited` with the wait otherwise. ATP spaces requests at least a
second apart and pauses briefly before each write. No cookies are sent,
and there are no captchas.

## References

- Articles: the id (`2045821`), a link (`https://dev.to/ben/my-post-1k2j`)
  or `USER/SLUG`. Links are read by their path, so they must be links
  of the community in use.
- Comments: the id code `comments` lists (`2n5ko`) or a link
  (`https://dev.to/ben/comment/2n5ko`).
- Users and organizations: the username, `@name`, a profile link, or
  the numeric user id.
- Tags: `rust`, `#rust` or a `/t/rust` link.
- `#N` works after any list.

## Commands

Articles are posts of kind `article` with the reactions as `likes`, the
comment count, the tags, the cover picture, the reading time in
minutes (`extra.reading_minutes`) and the organization they were
published under. Lists carry the article's description as text;
`read` has the whole Markdown.

### hot

`--category popular|rising|day|week|month|year|infinity`: the
community's popular list (the default), rising articles, or the most
liked of the last day, week, month, year or ever.

### feed

`--type latest|fresh`: the newest articles (the default), or new ones
that have few reactions yet.

### read

One article with its whole Markdown as text, the description in
`extra.description`, when it was published and last edited, and the
canonical link when it was first published elsewhere.

### comments

The article's whole discussion in one request, as threads in the page's
order with every reply inside. Comments are HTML turned into text; a
removed one reads `[deleted]`.

### replies

`replies ARTICLE COMMENT`: the replies under one comment.

### user, user-posts

A profile: name, bio, location, when they joined, website, GitHub and
Twitter names. `user-posts` lists a user's or an organization's
published articles, newest first.

### tag, tags

`tag NAME [--top day|week|month|year|infinity]` lists a tag's popular
articles, or the most liked of a period; `tags` lists the community's
tags, the most used first.

### drafts

Your unpublished articles, with their Markdown (needs the key).

### post

`post TEXT --title TITLE [--topic TAG]... [--visibility private]`, or
`post --article FILE.md` for the Markdown in a file: a new article,
published, or kept as a draft with `--visibility private`. Up to four
tags. Markdown that opens with Jekyll front matter (`title:`,
`published:`, `tags:`, `cover_image:`, `series:`, `canonical_url:`)
needs no `--title`, and Forem takes those fields from it. ATP shows the
title, tags and state and asks before sending; `-y` skips the question.
There is no `--image` or `--cover`: the API takes no uploads, so link
pictures in the Markdown (or `cover_image: URL`).

### edit

`edit ARTICLE [TEXT] [--title TITLE] [--topic TAG]... [--no-topics]
[--visibility public|private]`: changes one of your articles, published
or a draft (found among yours through `articles/me/all`); ATP shows each
change (`old → new`) and asks. `--visibility private` unpublishes an
article (it becomes a draft again) and `public` publishes a draft.
Front matter in the article's Markdown wins over `--title` and
`--topic`, so change those fields in the Markdown of such articles.

## Limits

No search (Forem's API has none), no deleting articles (unpublish them
with `edit --visibility private`), no comments, reactions, follows or
reading list, no image uploads, and no listing of other people's
drafts. Podcasts, videos and listings of the community are not read.
