Publishes a new post as your account. What it creates differs per
platform: a tweet, a pin (想法), an image note, a dynamic, a Reddit post ...
(the table in `atp guide interact`).

    atp x post "Hello" -i photo.jpg
    atp x post "Nice!" --reply-to 2104695667180380260
    atp xhs post "正文 #话题" --title 标题 -i cover.jpg
    atp reddit post "Body text" --title Title --topic r/test
    cat draft.md | atp bili post -

- TEXT may be left out when `--title` or an image is given; `-` reads it
  from standard input (not over MCP).
- `--title` sets a title; some platforms require one (Xiaohongshu,
  Reddit).
- `-i PATH` (repeatable) attaches images in order; each must be an
  existing file. Platforms cap the number (four on X).
- `--topic` (repeatable) attaches a topic, hashtag or community in the
  platform's form: a hashtag, a topic id, `r/sub`.
- `--reply-to POST` publishes a reply in that post's thread; `--quote POST`
  quotes or reposts it. Both take an id, link, `url` or `#N`.
- Where the platform offers them (its capabilities in `atp platforms`):
  `--video PATH` publishes a video and `--article PATH` an article from a
  Markdown or text file (`post-video`, `post-article`), with `--cover
  PATH`; `--cover-horizontal PATH` adds a landscape (4:3) cover where the
  platform keeps one beside the main cover (`post-cover-horizontal`);
  `--visibility public|friends|private` (`post-visibility`);
  `--ai` declares the content AI-generated with the platform's label
  (`post-ai-label`); `--schedule WHEN` publishes later: `+2h`, `+1d`,
  `2026-10-20 20:00` (local) or an RFC 3339 time (`post-schedule`);
  `--collection NAME|ID` (`post-collection`) and `--music ID`
  (`post-music`); `--chapters FILE` gives a video chapters
  (`post-chapters`); `--category ID` puts a video in one of the
  platform's categories (分区, `post-category`); `--location ID` tags
  the post with a place, by an id from the platform's place search
  (`post-location`); `--activity ID` joins one of the platform's
  official activities, by an id from its activity list
  (`post-activity`). Both take `#N` from the last list. Elsewhere they
  are hidden and refused.
- A chapters file holds one chapter a line: its start (`M:SS` or
  `H:MM:SS`), then its title, optionally after ` - ` or ` | `:

      0:00 Intro
      1:02 Setting up
      12:30 - Questions

  Times must rise from line to line; `-` reads the file from standard
  input (unless TEXT is read from there). Each platform's guide gives its
  rules (how many, how long, the first at 0:00).
- Douyin, TikTok, and Bilibili for videos, show what they are about to
  publish and ask before uploading anything; scripts pass `-y`.
- The result is an Action with the new post's `id` and `url`; both work as
  POST later (`read`, `delete`).
- Writes pause briefly first on platforms that flag bursts;
  `rate_limited` or `verification_required` mean slow down.

More: `atp guide interact`
