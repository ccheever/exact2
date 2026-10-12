# Recipe: the standard functions

Every function an expression may call; `bun exact.mjs contract vocab <name>` has one's rules (a
name Contract lacks, such as `sort`, prints what to write).

- **Text:** `trim(s)` · `toLowerCase(s)` ·
  `includes(s, t) -> bool` · `startsWith(s, t)` · `endsWith(s, t)` · `indexOf(s, t) -> number` ·
  `slice(s, start, end?)` · `split(s, sep) -> list<string>` · `replaceAll(s, find, with)` ·
  `toString(x) -> string` · `parseNumber(s) -> option<number>` · `encodeURIComponent(s)`.
- **Lists:** `length(x)` and `isEmpty(x)` (text too) · `map(xs, (x, i) => y) -> list<U>` ·
  `filter(xs, x => bool)` · `first(xs) -> option<T>` · `at(xs, i) -> option<T>` (negative
  from the end) · `slice(xs, start, end?)` · `concat(xs, ys)` · `includes(xs, x) -> bool` ·
  `indexOf(xs, x) -> number` · `join(xs, sep) -> string`.
- **Numbers:** `floor(n)` · `ceil(n)` · `round(n)` · `min(a, b)` · `max(a, b)`; the
  operators `+ - * / %`.
- **Formatting:** `formatNumber(n, "decimal" | "percent" | "compact")` ·
  `formatNumber(n, "currency", "USD")` · `toFixed(n, digits)` · `formatDecimal(cents, 2)` ·
  `formatDate(ms, offset, "medium" | "month-year" | "iso")` · `formatTime(ms, offset, "short")` ·
  `calendarDiff(from, to, "years" | "months") -> option<number>` · `t("key", name=value)`.
- **Time:** `performanceNow() -> number`, milliseconds since this launch; the date is
  `time.epochAtZero + performanceNow()` ([dates-and-days](dates-and-days.md)).
- **Routes:** `push(nav, location)` · `replace(nav, location)` · `back(nav)` ·
  `go(nav, location)` · `open(nav, location)` · `select(nav, tab)` · `stack(nav) -> list<Entry>` ·
  `top(nav) -> Entry` · `depth(nav)` · `params(nav, name) -> list<string>` ·
  `searchParam(entry, name)` · `path("route", args…)` · `encodeRouteSegment(s)`.
- **Requests:** `pending(x) -> bool` · `failed(resource) -> bool` ·
  `failure(resource) -> option<Failure>`.
- **Values:** `some(x)`, `none` · `Shape(field=value, …)` · `Shape(base, field=value)` ·
  `empty(field=value)` (a resource's placeholder).

Host commands are action statements, not functions: `focus(id)`, `blur()`, `showModal(id)`,
`close(id)`, `copyText(s)`, `openURL(url)`, `share(…)`, `haptic(…)`, `scrollIntoView(id)`
([grammar](../contract-grammar.md#host-commands)); in actions, `frame(id)`, `measure("id")` too.

What the web has and Contract doesn't is a composition, or the data module's job:

- With `p` a predicate `x => …`: `find` is `first(filter(xs, p))`, `some` is
  `includes(map(xs, p), true)`, `every` is `not includes(map(xs, p), false)`, `findIndex` is
  `indexOf(map(xs, p), true)`.
- A count chooses its noun by the number: `${n} ${n == 1 ? "book" : "books"}`, never "1 books"
  (`lower-count-plural`).
- Sorting, grouping, sums and other maths (`sort`, `reduce`, `pow`, `abs`) are the data module's:
  it answers the view the result. Upper case is CSS: `text-transform="uppercase"`. An app's own
  `fn` names a reusable expression.
