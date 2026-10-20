12306 (www.12306.cn), China Railway's ticket site, read the way its
query page reads it: stations, the trains between two stations on a day
with the seats left in each class, a train's stops and its fares.
ATP only looks: it never books, orders, waitlists (候补) or pays.

`ctrip trains` and `ctrip transfers` read Ctrip's resale pages (fares
with every train, plans with a change); `12306` is the railway's own
data: the seats left per class as 12306 shows them (有, 无, a count), a
train's stops and its official fares.

## Logging in

There is nothing to log in to: every command works without an account,
and ATP offers no login, orders, passengers or bookings. `atp 12306
status` therefore always reports "not logged in". No environment
variables are read.

Each run first opens 12306's query page once, as a browser would: it
sets the anonymous session cookies the queries want (kept for the run
only) and names the current address of the ticket query, which 12306
changes from time to time (ATP also follows the `c_url` 12306 answers
with when it moved). The station list is kept for a week. Requests are
spaced at least 3 seconds apart.

12306 publishes no limits. It turns away frequent queries, and some
networks, with its error page or a "busy" web page instead of data:
ATP stops with `rate_limited` ("12306 turned the request away") or
`upstream_error`, and waiting a few minutes helps. There are no
captchas on these queries; logins and orders, which have them, are not
offered.

## References

- Stations: a name (`上海虹桥`, `上海站`), a city's own name, which is its
  main station (`上海` → 上海, `SHH`), the telecode (`AOH`), the pinyin
  (`shanghaihongqiao`) or its initials (`shhq`).
- Trains: the code (`G102`, `K1`, `1461`), or a train of a `trains` list
  by `#N` or its `id`: `G102/AOH-VNP/2026-10-25` (the code, the
  telecodes of the stations it was listed between, the day).
- Days: `--date YYYY-MM-DD`; today in Beijing by default, or the day of
  a listed train. 12306 sells about two weeks ahead and says so for
  days outside that (`invalid_input` with its words).

## Commands

### stations

`stations KEYWORD`: stations whose name, telecode, pinyin, initials or
city match, exact matches first; each with its telecode, pinyin,
initials and city.

### trains

`trains FROM TO [--date DAY] [--type G,D] [--available]` (also
`tickets`): every train of the day between two stations, in 12306's
order. Like its site, 12306 answers with the trains between any
stations of the two cities (上海虹桥 → 杭州东 lists 上海南 → 杭州南 too);
each train's `extra.from` and `extra.to` say which. Each is a post titled `G102 上海虹桥 06:26 → 北京南 12:29` (`+1`
when it arrives a later day), its time the departure, its text the
travel time and the seats left per class as 12306 shows them (`有`:
plenty, `无`: none, a number: that many, `*`: not on sale yet), and
`extra` with the train number, both stations, the train's origin and
terminus, whether 12306 sells it on the web (`bookable`) and the seats.
A status other than 预订 (`列车停运` …) leads the text. `--type` keeps
trains whose code starts with those letters (trains numbered without
a letter then drop out); `--available` keeps trains with a seat left.
The list has no fares: `price` asks for one train's.

### train

`train TRAIN [--date DAY]` (also `stops`): the train's stops that day,
each with its number, arrival, departure and stop time, the train's
class (高速, 动车 …), and for a train of a list the stations of its
segment marked (`extra.stops[].in_segment`).

### price

`price TRAIN [FROM TO] [--date DAY]`: the train's fares between two of
its stations per seat class, with the seats left, the lowest fare as the
price. FROM and TO may be left out for a train of a `trains` list. It
queries the day's trains between the two stations first, then the
fares: three requests with the query page.

## Limits

Read-only and anonymous: no login, orders, passengers, bookings,
waitlists, seat choice, refunds or changes. Adult tickets only (no
student fares), no plans with a change (`ctrip transfers` has them),
and no fares in the train list (one `price` per train).
