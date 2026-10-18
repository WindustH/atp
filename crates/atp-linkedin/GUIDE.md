LinkedIn's jobs, read the way its public jobs pages show them to a
visitor without an account: the job search by words, place, date posted
and company, a job's page (description, criteria, pay where shown, how
to apply), companies' pages and their open jobs. ATP only reads: it
never applies, saves, follows or sends anything, and it leaves people
(recruiters, the hiring team, employees) out of everything it prints.

## Logging in

There is nothing to log in to: every command works without an account,
and `atp linkedin status` always reports "not logged in". No environment
variables are read.

ATP does not sign in to LinkedIn on purpose. LinkedIn's User Agreement
forbids automated access, and LinkedIn restricts or bans accounts it
suspects of it; a visitor has no account to lose. LinkedIn's logged-in
API (Voyager: the logged-in search, saved and applied jobs) is therefore
not supported: the references it was checked against pin down only its
search cards, there is no stable call for saved or applied jobs, and it
cannot be tested without putting an account at risk. Should it come, it
will be read-only, paced, and with a personal account at the user's own
risk.

The cookies LinkedIn hands a visitor (`JSESSIONID`, `bcookie`, `lidc` …)
live for one command and are never saved. The numbers a company's jobs
are searched by, read from its page, are kept a month in the cache.

Risk control: ATP keeps at least 4 s between requests (with random
jitter) and never retries a refused one. LinkedIn does not publish its
limits for visitors; when an address reads too much it answers HTTP 429
or its own HTTP 999 (`rate_limited`), sends the visitor to its sign-in
wall (`not_authenticated`: logging in is not the way out, waiting is) or
to a security check (`verification_required`). ATP never answers such a
check and does not open it in a browser, as its requests carry no
session a browser could clear. Keep `-n` modest and wait an hour or more
after any of these.

## References

- Jobs: the link lists print, `https://www.linkedin.com/jobs/view/<id>`;
  any job link as LinkedIn writes it
  (`https://de.linkedin.com/jobs/view/<title>-at-<company>-<id>`, any
  country's host); links of search and collection pages with the job
  open beside them (`…?currentJobId=<id>`); `urn:li:jobPosting:<id>` (or
  `urn:li:fsd_jobPosting:<id>`); the id alone.
- Companies: `https://www.linkedin.com/company/<name>` (what jobs give as
  their `author`; any page under it, such as `/jobs`), the name in it
  (`nvidia`), the company's number (`company_id` of a job or a company),
  `urn:li:company:<n>`, `urn:li:organization:<n>` or
  `urn:li:fsd_company:<n>`.
- Places (`jobs --location`): words, as LinkedIn's own search box takes
  them (`Berlin`, `San Francisco Bay Area`, `Germany`); `jobs --geo`: the
  number after `geoId=` in the address of LinkedIn's job search
  (`92000000` is the whole world).
- `#N` works after any list.

## Commands

### search

`search WORDS` is LinkedIn's job search by words, 10 jobs a page, where
LinkedIn takes the visitor to be (it picks a country by the address the
request comes from). `jobs` names a place, a time and companies.

### jobs

`jobs [WORDS]` narrows the search the ways LinkedIn's search does for a
visitor:

- `--location PLACE` (words) or `--geo ID` (LinkedIn's number for a
  place), not both;
- `--posted day|week|month`: posted in the past 24 hours, week or month;
- `--company C`: a company's link, the name in it or its number; several
  separated by commas or given again. A name costs one read of the
  company's page the first time (its numbers are kept a month); a number
  costs nothing.

LinkedIn's own pages also offer workplace (on-site, remote, hybrid),
experience level, job type and the order (most recent first), but the
search a visitor's pages load ignores them: asked for internships it
answers with staff engineers, asked for the most recent it answers in
its own order (checked 2026-10-18). ATP does not offer filters that
would be silently ignored; a job's page (`read`) tells its seniority
level and employment type.

Jobs come in LinkedIn's order of relevance, so `--since` / `--until`
keep the jobs in the window from the pages read, and a page holding only
older jobs ends the listing; `--posted` narrows the search itself.
LinkedIn goes no further than the first 1000 jobs of a search: a listing
that reaches them says so, and `--cursor` past them is refused.

    atp linkedin jobs rust --location Berlin --posted week
    atp linkedin jobs --company nvidia,krakenfx --posted month -n 50

### read

A job's page: the title, the whole description, the company (`company`;
the `author`, with its page and logo; `company_id`), the place (`city`),
when it was posted (`posted` as shown; `created_at` from it: `15 hours
ago` as a time, days and longer as the day), the applicants, the
criteria (seniority level, employment type, job function, industries),
the pay where there is some, and how to apply: `offsite_apply` (true:
on the company's site; false: LinkedIn's Easy Apply) and `apply_url`,
the company's application link where the page gives it, else the job's
page. A job that no longer takes applications has `closed: true`.

ATP never applies, with Easy Apply or otherwise; there is no `apply`
command.

### user

A company's page: its name, industry, tagline (`extra.tagline`), "About
us" (`bio`), headquarters (`location`), followers, website, size, type,
founding year and specialties, its number (`company_id`) and the numbers
its jobs are searched by (`jobs_company_ids`). The page's employees,
posts and similar pages are not read.

    atp linkedin user https://www.linkedin.com/company/nvidia

### user-posts

A company's open jobs, all over the world, in LinkedIn's order: the
search by the numbers of the company's "See jobs" link, which may name
pages of its own beside it (a subsidiary, a brand). The first use of a
name reads the company's page once; a number goes straight to the
search.

    atp linkedin user-posts https://www.linkedin.com/company/nvidia -n 30
    atp linkedin user-posts 3608

## Job posts

Jobs (`kind: job`) carry the job keys every job platform uses in `extra`:

- `company`, and `city`: the place as LinkedIn writes it (a city with its
  region and country, a metro area, or a whole country);
- `remote`: `true` when the title or the place says remote; absent
  otherwise, as LinkedIn's pages for visitors do not tell the workplace;
- `salary`, `salary_min` / `salary_max`, `salary_currency` (ISO: `USD`,
  `EUR`, `GBP`, `INR` …; LinkedIn's `¥` is the yen, its `CN¥` the yuan)
  and `salary_period`: the base pay LinkedIn shows; else pay named in
  the title (`($185K - $300K)`); else, on job pages, the first range of
  pay in a line of the description that speaks of pay (`The base salary
  range is 184,000 USD - 287,500 USD`; single figures there are not
  taken). Pay without a period is taken by its size: below 300 an hour,
  from 20,000 a year, a month between;
- on job pages: `experience` (the seniority level as shown, unless "Not
  Applicable"), `employment_type` as shown and `job_type` where it fits
  (`full-time`, `part-time`, `contract`, `intern`; not Temporary,
  Volunteer or Other).

LinkedIn's own keys: on lists, `tags` holds LinkedIn's notes on a job
(`Actively Hiring`, `Be an early applicant`) and `created_at` the day it
was listed; on job pages, `job_function`, `industries`, `applicants` (the
count, when LinkedIn gives one) and `applicants_text` (`Over 200
applicants`), `posted`, `reposted`, `company_id`, `offsite_apply`,
`apply_url` and `closed`.

People are never read: not the hiring team, the job's poster, "see who
was hired", referrals or employees. `raw` holds the parts of a page that
were read (title, company, place, criteria …), not the page.

## Not supported

- Logging in, and with it the logged-in search, job details, saved and
  applied jobs (see Logging in).
- Applying (Easy Apply or a company's form), saving jobs, following
  companies, messages and connection requests: ATP only reads.
- People: profiles, people search, recruiters.
- Filtering by workplace, experience level or job type, and the most
  recent first: LinkedIn's search ignores them for visitors.

## Limits

- 1000 jobs per search at most; narrow it to see others.
- Places are words LinkedIn interprets; it may widen a place it does not
  know.
- Times on job pages are LinkedIn's rounded ages (`2 weeks ago`), so a
  job's `created_at` from `read` can differ by days from the date a list
  gives it.
- `remote` is known only where the words say so.
- Pay appears where LinkedIn or the employer shows it, mostly for places
  whose laws ask for it.
- LinkedIn's limits for visitors are not published; reading many pages
  in a row ends in a wall for a while (see Logging in).
