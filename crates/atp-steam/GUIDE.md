Steam through the store's open JSON API (search, app pages, the front
page's lists, user reviews) and, with a key you make, the Steam Web API
(public profiles and the games they own). ATP only reads: nothing is
bought, wishlisted, reviewed or added to a library.

## Logging in

The store needs no account: `search`, `hot`, `read`, `comments`,
`reviews` and `prices` work without logging in.

Profiles (`user`, `whoami`) and owned games (`games`) come from the Steam
Web API, which wants a key:

1. Sign in on https://steamcommunity.com/dev/apikey and register a key
   (Steam asks for a domain name; any name of yours will do). Steam
   gives keys only to accounts that are not limited (an account that
   has spent at least US$5 in the store).
2. Run `atp steam login --token 'API_KEY|PROFILE'`, where PROFILE is
   your profile: its SteamID64 (`76561197960287930`), its link, or its
   custom name (the part after `/id/`). ATP looks the profile up with
   the key and keeps both in the session file.

`STEAM_API_KEY` supplies a key for one run instead (`user` and `games
USER` work with it; `whoami` and `games` without USER need the profile
saved at login). A key lasts until you revoke it on the same page; a
revoked or wrong key gives `not_authenticated` ("Steam refused the Web
API key"). Valve's Web API terms allow a key 100,000 calls a day.

Steam publishes no limit for the store's JSON API; it answers bursts
with HTTP 429 (`rate_limited`), after which waiting a minute or two
helps. ATP spaces all requests at least 1.5 seconds apart. No cookies
are sent and there are no captchas to pass.

Settings, read from the environment:

- `STEAM_CC`: the storefront country of prices and availability (two
  letters: `us`, `cn`, `de`, `jp`, `tr` …; `us` by default).
- `STEAM_LANGUAGE`: the language of store texts (Steam's names:
  `english`, `schinese`, `tchinese`, `japanese`, `koreana`, `german`,
  `french`, `russian`, `spanish` …; `english` by default).

## References

- Apps (games, DLC, software): the id (`1245620`), `app/620`, a store
  link (`https://store.steampowered.com/app/620/Portal_2/`) or a
  community link (`https://steamcommunity.com/app/620`). A bare number
  of up to three digits means `#N` of the last list when there is one,
  so write the old apps with small ids as `app/620`, `app/730`.
- Profiles: a SteamID64, a `https://steamcommunity.com/profiles/<id>`
  or `https://steamcommunity.com/id/<name>` link, `@name` or the custom
  name.
- `#N` works after any list.

## Commands

Apps are posts: `kind` is Steam's type (`game`, `dlc`, `demo`,
`music`, `software` …; `app` in lists), the price in `price` (with the
price before a discount and the discount), the system list in
`extra.platforms`.

### search

The store's quick search for a name: one short list of apps with their
price in `STEAM_CC`, the systems they run on and their Metacritic score.
There are no further pages.

### hot

`--category top-sellers|specials|new-releases|coming-soon`: the lists of
the store's front page (top sellers by default) in `STEAM_CC`, each app
once, with discounts and when they end (`extra.discount_ends`).

### read

An app's page in `STEAM_CC` and `STEAM_LANGUAGE`: name, price, release
date (the post's time; the date as shown in `extra.release_date`, and
`extra.coming_soon`), genres as tags, the short and the long
description as text, developers (the first is the author), publishers,
systems, categories, Metacritic score, achievements, DLC count, age
rating, languages, website, the header picture, screenshots and the
trailers that have a file link. The summary of the reviews in every
language goes into `extra.reviews` (`Very Positive`, counts, share of
positive ones) and the metrics (`likes`: positive reviews, `comments`:
all reviews). Apps the storefront does not sell there give `not_found`.

### comments

The app's reviews in every language, 100 a page at most, through
Steam's cursor. `--sort helpful|recent|updated|positive|negative`:
Steam's most helpful first (the default), newest first, most recently
changed first, or only positive or only negative ones (most helpful
first). Each review carries `extra.recommended`, hours played when it
was written and in all (`extra.hours_at_review`, `extra.hours_total`),
funny votes, the language, and whether the game was bought on Steam;
helpful votes are `likes`. Authors are known by SteamID only (Steam's
review API gives no names). `reviews` has more filters.

### reviews

`reviews APP [--sort helpful|recent|updated] [--type all|positive|negative]
[--language NAME|all] [--purchase all|steam|other] [--days N]`: reviews
as `comments` lists them, in one language (Steam's names, as for
`STEAM_LANGUAGE`), of one kind, by where the game was bought (`other`:
keys bought elsewhere), or the helpful ones of the last N days only.

### prices

`prices APP… [--cc us,cn,tr]`: each app's price in each storefront,
one post per app and country (`extra.cc`), as `STEAM_CC` alone without
`--cc`. The first storefront reads each app's page; the others ask for
the prices of all the apps at once. Free apps say so; apps not sold in a
storefront show no price.

### user

A profile through the Web API (needs a key): name, custom name, avatar,
country, when the account was made, whether it is online or playing
something (`extra.state`, `extra.playing`) and whether the profile is
public. Private profiles show their name and picture only.

### games

`games [USER] [--recent] [--sort playtime|recent|name]`: the games a
profile owns (yours without USER), most hours first, with hours played
in all and in the last two weeks and when it was last played; `--recent`
lists the games played in the last two weeks. Profiles that keep their
game details private give `permission_denied`.

## Limits

No purchases, cart, wishlist, library changes, reviews, ratings or
community writing. No Steam Community reads beyond profiles and owned
games (no workshop, discussions, screenshots, friends or a user's
reviews), no store search beyond its first list, and no downloads
(trailers that stream only are left out).
