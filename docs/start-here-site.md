# Start here: a website with Exact

The only required reading for a website: pages at URLs, served and read in a browser. A phone
or desktop app reads [start-here.md](start-here.md) instead. The rest is lookup.

## The idea

You write HTML's elements and CSS's properties, and the build makes the site: each page
rendered ahead or per request, sent whole, then a small script for its links and controls.

- **A tag is the HTML element it names:** `main`, `nav aria-label`, a `header` and `footer`
  beside `main`, `section aria-labelledby`, `h1`–`h6`, `table` (`caption`, `thead`, `tr`,
  `th scope="col"`, `aria-sort`), `dl` (`dt`, `dd`), `select`, `input type="search"`, `link
  href` (an `a`), `image "src" alt=""` (an `img`), `text "…"` (words; `role="heading"
  aria-level=2` makes them a heading). A `list` supplies item roles for links or cards and
  keeps each link's role.
- **An attribute is a CSS property**, with CSS's names and values: `padding="8px 16px"`,
  `display="grid" grid-template-columns="repeat(auto-fill, minmax(9rem, 1fr))"`,
  `border-bottom="1px solid var(--line)"`. A bare number is pixels; quote units and
  keywords. `row` and `column` are a flex `view`, `view` a `div`. `style Card` with its rows,
  then `class=Card`, is a class.
- **A design's tokens** come in with `bun exact.mjs tamber import css tokens.css`, which
  writes `app.tamber`; a row names one as `var(--ink)` or `ink`, also inside `calc()`.
- **A page is a route**, so Back, reload, a shared link and a new tab work.

## The workflow

```sh
bun <exact2>/scripts/exact.mjs new site && cd site
bun exact.mjs pattern site   # a tested website: start from it
```

The pattern writes `app.contract` (the pages), `app.ts` (the data module), `app.test.contract`
(its tests), `server.ts` (a stand-in for its API) and `.env` (the API's origin). Its comments
say why each line is so: keep its shape, replace its content. Read
[recipes/website.md](recipes/website.md) next: when a page is made, serving it, the markup.

|`bun exact.mjs …`|What it does|
|---|---|
|`web-build --out dist`|Compiles and builds the site. A Contract error or an `app.ts` type error fails it, naming the line: it is the one check after an edit (`npm run build`).|
|`serve dist`|Serves it, rendering `render=request` pages per request, and builds again on an edit (`npm start`).|
|`test web`|Runs `app.test.contract` in Chrome.|

Keep scratch files in `.exact/` and helper scripts in `tools/`: neither makes the build stale.

## The language

```contract
routes nav
  home "/" render=request
  item "/item/:id" render=request
  notfound render=build

shape Item
  id: string
  name: string

fn label(n: number): string = `${n} ${n == 1 ? "item" : "items"}`

component App
  resource items = loadItems() as shape Item[] else empty()
  derive q = searchParam(top(nav), "q")
  derive shown = sortBy(items.filter(i => i.name.toLowerCase().includes(q)), i => i.name)
  action follow(url: string)
    nav = go(nav, url)
  action search(v: string)
    nav = replace(nav, path("home") + `?q=${encodeURIComponent(v)}`)
  view
    view navigationKey=`${top(nav).id}` navigate=follow
      for e in stack(nav) key=e.id
        if e.name == "home"
          main navigationKey=`${e.id}`
            head title="Items"
            input type="search" value=q input=search aria-label="Search" testId="q"
            text label(shown.length) testId="count"
            list
              for i in shown key=i.id
                link href=path("item", i.id)
                  text i.name
```

|Declaration|Where|Meaning|
|---|---|---|
|`shape Name` with typed fields|file|a record: `string`, `number`, `bool`, `T[]`, `option<T>`, another shape|
|`fn name(a: T): U = expr`|file|a function of its arguments|
|`style Name` with rows; `class=Name`|file|a class of style rows; an element's own rows win|
|`routes nav`|file|the pages: `name "/path/:param" render=…`, nested to stack, `notfound`|
|`component Name` with `props`|file|the first is the root; another is called as a tag, `Card(item=i)`|
|`state x = value`|component|a stored value an action assigns|
|`derive x = expr`|component|a value computed from others, kept current|
|`resource x = source(args) as shape T else empty()`|root|a read of the data module, again when `args` change; `pending(x)`, `failed(x)`, `failure(x)`|
|`mutation x as shape T`, `send x = source(args)`|root|a write with a reply (`refreshes y` reads `y` again)|
|`action name(arg: T)`|component|what an event runs: assignments and sends, landing when it ends|

A route's entry `e` (from `for e in stack(nav)`) has `e.name`, `e.id` and `e.params.id`;
`top(nav)` is the page shown. `path("item", id)` writes a route's path, `go(nav, url)` follows
a link, `replace(nav, url)` changes the location in place, `searchParam(top(nav), "q")` reads
the query and `fragment(top(nav))` what follows `#`.

- **Syntax.** Indent with spaces. Subtract with spaces (`a - b`): names hold hyphens. A view
  chooses with `if`/`else` and repeats with `for x in xs key=x.id`; there is no `form` (a
  field's Enter is its `submit`).
- **Expressions** are TypeScript's: what compiles means what it means there, and what does
  not is refused with what to write. String and array methods are methods (`s.trim()`,
  `xs.filter(x => …)`, `xs.length`); the rest are functions (`first(xs)`, `round(n)`,
  `formatNumber(n, "decimal")`). No truthiness; absence is an option (`none`, `some(x)`,
  `match x { case some(v) => …, case none => … }`), not `null`; a value never changes
  (`xs.concat([x])`); an order is `sortBy(xs, x => x.key)`, stable (`"desc"` third).
- **The query** holds a page's filters and sort, so Back, reload and a shared link keep them.
- **A page's head:** `head title=… description=… status=404` inside the page. A path no route
  matches is the `notfound` route, which `serve` answers 404. `status` is a literal 404, 410 or
  503; put its head behind a data condition when the status depends on an answer. The default
  server waits for such a page before committing a browser navigation's HTTP status.

**The data module**, `app.ts`, exports `answer`, which `sources` implements: each source is
`(args, store, storage, native)`, `args` its arguments in order (`loadItem: async ([id]) => …`).
It answers with exactly the declared shape: an extra field fails a resource, so build each
record field by field. Type a source's result (`Promise<Result<'loadItems'>>`) so the build
names one.
It reaches only what its `grants` name: `export const grants = \`net.fetch ${API}\``, with
`const API = import.meta.env.API_BASE_URL` (app.json's `env`). A failed read should throw,
failing its resource. It imports its own `.ts` and pure-JS npm packages; one needing Node
(`fs`, `Buffer`) or the DOM is refused by name.

## CSS that is not here

Each is refused at build, with the remedy; write the remedy the first time.

|CSS|Write|
|---|---|
|`border-collapse`|`border-spacing=0` on the `table`, and a `border-bottom` on each cell|
|`display="inline"`, `inline-block`|leave `display` off inside a `row`, whose items are as wide as their content; in block flow, put the box in a `row`. A `text` inside a `text` is a run of its line|
|`display="inline-flex"`|`display="flex"`, as an item of a `row` to size it to its content|
|`list-style`, `display="list-item"`|no marker is drawn: a bullet is a `text "•"` before the item's text in a row|
|`background="…"`|`background-color` (and `background-image`)|
|`width="100%"` with padding|it overflows, as in CSS: drop the width; a `flex=1` field takes `min-width=0`|

## Look it up

This guide, the pattern and the website recipe are the language a site needs; look up a
name, not a chapter. `bun exact.mjs contract vocab <name> …` answers for any tag, property,
function or recipe in one call (`contract vocab head select table media`); `contract vocab`
alone prints the card, every name on one page. `bun exact.mjs docs <heading words>` prints the
one section of the long guides that answers a question. Recipes: `website`, `web-look` and `responsive-columns` (wide layouts), `select`,
`fetch-loading-error`, `rest-backend`, `tamber` (a brand), `tests`.
