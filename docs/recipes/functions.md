# Recipe: the standard functions

Every function an expression may call; `bun exact.mjs contract vocab <name>` has one's rules, or
what to write for a name Contract lacks. The web's methods are called on their value
(`s.trim()`, `xs.length`), the rest as functions (`first(xs)`).

- **Text:** `s.trim()` · `s.toLowerCase()` · `s.toUpperCase()` ·
  `s.includes(t)` · `s.startsWith(t)` · `s.endsWith(t)` · `s.indexOf(t) -> number` ·
  `s.slice(start, end?)` · `s.split(sep) -> string[]` · `s.replaceAll(find, with)` ·
  `x.toString()` · `parseNumber(s) -> option<number>` · `encodeURIComponent(s)`.
- **Lists:** `x.length` and `x.isEmpty()` (text too) · `xs.map((x, i) => y)` ·
  `xs.filter(x => bool)` · `first(xs) -> option<T>` · `xs.at(i) -> option<T>` ·
  `xs.slice(start, end?)` · `xs.concat(ys)` · `xs.includes(x)` ·
  `xs.indexOf(x) -> number` · `xs.join(sep)` · `sum(xs)` · `sortBy(xs, x => key)`, stable ·
  `range(n)`.
- **Numbers:** `floor(n)` · `ceil(n)` · `round(n)` · `min(a, b)` · `max(a, b)` ·
  `sin(rad)` · `cos(rad)` · `+ - * / %`.
- **Formatting:** `formatNumber(n, "decimal" | "percent" | "compact")` ·
  `formatNumber(n, "currency", "USD")` · `n.toFixed(digits)` · `formatDecimal(cents, 2)` ·
  `formatDate(ms, offset, "medium" | "month-year" | "iso")` · `formatTime(ms, offset, "short")` ·
  `formatDay(then, now, offset, "day" | "list")` ·
  `calendarDiff(from, to, "years" | "months")` · `t("key", name=value)`.
- **Time:** `performanceNow()`, ms since launch; the date is
  `time.epochAtZero + performanceNow()` ([dates-and-days](dates-and-days.md)).
- **Routes:** `push(nav, location)` · `replace(nav, location)` · `back(nav)` ·
  `go(nav, location)` · `open(nav, location)` · `select(nav, tab)` · `stack(nav) -> Entry[]` ·
  `top(nav) -> Entry` · `depth(nav)` · `params(nav, name) -> string[]` ·
  `searchParam(entry, name)` · `path("route", args…)` · `encodeRouteSegment(s)`.
- **Requests:** `pending(x)` · `failed(resource)` ·
  `failure(resource) -> option<Failure>`.
- **Values:** `some(x)`, `none` · `Shape(field=value, …)` · `Shape(base, field=value)` ·
  `empty(field=value)` (a resource's placeholder).

Host commands are statements: `focus(id)`, `blur()`, `showModal(id)`,
`close(id)`, `copyText(s)`, `openURL(url)`, `share(…)`, `haptic(…)`, `scrollIntoView(id)`
([grammar](../contract-grammar.md#host-commands)); in actions, `frame(id)`, `measure("id")` too.

What the web has and Contract lacks is a composition or the data module's:

- With `p` a predicate `x => …`: `find` is `first(xs.filter(p))`, `some` is
  `xs.map(p).includes(true)`, `every` is `not xs.map(p).includes(false)`, `findIndex` is
  `xs.map(p).indexOf(true)`.
- A count's noun follows the number: `${n} ${n == 1 ? "book" : "books"}`, never "1 books"
  (`lower-count-plural`).
- Grouping, folds and other maths (`reduce`, `pow`, `abs`) are the data module's.
  On screen, upper case is `text-transform="uppercase"`; `s.toUpperCase()` is for a string sent
  elsewhere (`share`, data).
