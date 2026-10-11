# Recipe: the standard functions

Every function an expression may call, one line each. For one function's rules and an
example: `bun exact.mjs contract vocab <name>` (a name Contract lacks, such as `sort`
or `pow`, prints what to write instead).

- **Text:** `length(s) -> number` · `isEmpty(s) -> bool` · `trim(s)` · `toLowerCase(s)` ·
  `includes(s, t) -> bool` · `startsWith(s, t)` · `endsWith(s, t)` · `indexOf(s, t) -> number` ·
  `slice(s, start, end?)` · `split(s, sep) -> list<string>` · `replaceAll(s, find, with)` ·
  `toString(x) -> string` · `parseNumber(s) -> option<number>` · `encodeURIComponent(s)`.
- **Lists:** `length(xs)` · `isEmpty(xs)` · `map(xs, (x, i) => y) -> list<U>` ·
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
- **Layout, in actions only:** `frame(id)` · `measure("id")` · `elementFromPoint(x, y)`.

Host commands are action statements, not functions: `focus(id)`, `blur()`, `showModal(id)`,
`close(id)`, `copyText(s)`, `openURL(url)`, `share(…)`, `haptic(…)`, `scrollIntoView(id)`
([grammar: host commands](../contract-grammar.md#host-commands)).

What the web has and Contract doesn't is a composition, or the data module's job:

```contract
shape Book
  id: string
  title: string
  done: bool

component Shelf
  state books = [Book(id="1", title="Middlemarch", done=true), Book(id="2", title="Emma", done=false)]
  derive found = first(filter(books, b => b.id == "2"))
  derive anyDone = includes(map(books, b => b.done), true)
  derive allDone = not includes(map(books, b => b.done), false)
  derive firstUnread = indexOf(map(books, b => b.done), false)
  view
    column padding=16 gap=8
      text (match found { case some(b) => b.title, case none => "Not found" }) testId="found"
      text `${length(filter(books, b => b.done))} of ${length(books)} read` testId="count"
      text (anyDone and not allDone ? `Next: book ${firstUnread + 1}` : "") testId="next"
```

```contract-test
test "find, some, every and count are compositions"
  expect text "found" == "Emma"
  expect text "count" == "1 of 2 read"
  expect text "next" == "Next: book 2"
```

- `find` is `first(filter(…))`, `some` is `includes(map(…), true)`, `every` is
  `not includes(map(…), false)`, `findIndex` is `indexOf(map(…), true)`.
- Sorting, grouping, sums and other maths (`sort`, `reduce`, `pow`, `abs`) happen in the
  data module, which answers the view the result. Upper case is CSS:
  `text-transform="uppercase"`. An app's own `fn` names a reusable expression.
