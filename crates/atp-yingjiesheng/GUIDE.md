Yingjiesheng (应届生求职网, yingjiesheng.com), 51job's site for students
and new graduates, for its campus recruitment talks (宣讲会): companies
presenting themselves and their openings at universities. ATP lists them
by words, province, city, school, company, day and industry, and reads a
talk with its venue, times and announcement. Its job search is not
supported yet.

## Logging in

There is nothing to log in to: every command works without an account,
and ATP neither books talks (预约) nor reads anything of an account.
`atp yingjiesheng status` therefore always reports "not logged in". No
environment variables are read.

Risk control: ATP keeps at least 2 s between requests; a talk's `read`
takes two (the talk's page and the mobile site's). The site does not
publish its limits. Parts of the site sit behind Aliyun's firewall (its
job pages, and pages of www such as its 404 page); a page that comes back
as its check is
`verification_required` with the page, which ATP neither answers nor
opens in a browser. ATP does not follow redirects either: a talk that is
gone sends to www's 404 page, behind that firewall, and is reported as
`not_found`.

## References

- Talks: their number (`6299612`), or a link holding it:
  `https://my.yingjiesheng.com/xjh-006-299-612.html` (what lists print:
  the number in groups of three, padded to nine digits) or the mobile
  site's `https://m.yingjiesheng.com/xuanjianghui/xjh_6299612`.
- Companies: their number (`1213060`), or the link of their talks,
  `https://my.yingjiesheng.com/xuanjianghui_cid_1213060.html` (the
  `author` of a talk `read` shows).
- `#N` works after any list.

## Commands

### search

`search WORDS` lists the talks whose company, school or venue holds the
words (the site's 关键字检索), 25 a page, from today on in the site's
order. `talks` narrows further.

### hot

The talk list as the site opens it: every talk from today on, 25 a page,
in the site's order.

### read

A talk's page: the title, the company (as `author`, with the link of its
talks), city, school, venue (`venue`), the remark (`remark`), the
announcement (`text`) and the page the site took it from (`source_url`,
usually the school's career site). The page shows the time as a picture,
which ATP does not read; the mobile site's page of the same talk gives
the start and end (`starts`, `ends`; `created_at` is the start), the
province and the company's industry, and the city and announcement when
the talk's page has none. Without the mobile page (it may fail on its
own) the talk comes without them.

### user-posts

`user-posts COMPANY` lists a company's talks from today on.

### talks

`talks [WORDS]` is the talk list with the site's filters; without words
it lists every talk they let through, from today on.

- `--province`: a province or region by name (浙江, 浙江省) or its code
  from `provinces`.
- `--city`: a city by the site's number, as listed talks carry it
  (`extra.city_id`); the site has no list of them.
- `--school`: a school by the site's number, from `schools PROVINCE` (the
  province's hot schools) or a listed talk (`extra.school_id`).
- `--company`: a company by its number or the link of its talks.
- `--date 2026-10-22|today|tomorrow`: that day only.
- `--industry`: an industry by name or part of one (互联网, 银行, 能源 …)
  or its code from `industries`; the site files companies under them.

    atp yingjiesheng talks --province 浙江 --date tomorrow
    atp yingjiesheng talks --school 7 -n 50

### provinces, industries

The codes of `talks --province` and `--industry`, as the list's page
offers them.

### schools

`schools PROVINCE` lists the hot schools (热门学校) the province's page
offers, with their numbers for `talks --school`.

## Talks

Talks (`kind: talk`) carry in `extra`:

- `company` (the job key; on `read` the `author` is the company, with
  its number and the link of its talks);
- `city` and `city_id`, `school` and `school_id`, `venue`;
- in lists `date` (the day; `created_at` is that day, shown without a
  time);
- on `read` `starts` and `ends`, `province`, `industry`, `remark` and
  `source_url`.

Lists give the day of a talk but not its time, which the list's page
shows only as a picture; `read` has it.

## Limits

- No job search: the site's job search (youngapi.yingjiesheng.com) signs
  each request, and whether ATP should make those signatures is not
  decided yet. Its job pages (q.yingjiesheng.com) sit behind Aliyun's
  firewall and are not read.
- No accounts: no booking talks, no résumés, no applications.
- Cities only by number, schools only from a province's hot schools or
  from listed talks: the site lists neither in full.
- `--since` / `--until` do not apply to talks; `--date` picks a day.
