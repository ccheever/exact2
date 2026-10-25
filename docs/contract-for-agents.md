# Contract: a complete working guide for agents

> **Lookup reference; start at [start-here.md](start-here.md).** Building an app needs
> only that page, and a [recipe](recipes/) answers most questions after it. Open this
> guide at a section for what no recipe covers, or grep it; don't read it front to back.

Use this guide to author, change, inspect, and verify a current Exact application.
It covers the language implemented on `main` on 2026-10-02. Use the [human guide](contract-for-humans.md)
for explanations and complete examples, and the [grammar reference](contract-grammar.md)
for exact forms, built-in functions, tags, and event payloads.

The compiler and executable fixtures are authoritative. Read the repository's
`AGENTS.md`, `rules/RULES.md`, and `rules/DEFERRED.md` before making changes here.
This guide is documentation, not an additional policy layer. Documents in
`llp/research/` describe a predecessor and do not establish current capabilities.

## Contents

- [Orient and choose the right layer](#orient-and-choose-the-right-layer)
- [The implementation loop](#the-implementation-loop)
- [Language inventory](#language-inventory)
- [Values, expressions, and functions](#values-expressions-and-functions)
- [State and action semantics](#state-and-action-semantics)
- [Composition and lifetime](#composition-and-lifetime)
- [Data requests and side effects](#data-requests-and-side-effects)
- [Views, layout, and interaction](#views-layout-and-interaction)
  - [Tabular numbers](#tabular-numbers)
  - [Sound, MIDI and media](#sound-midi-and-media)
- [Routes and web documents](#routes-and-web-documents)
- [Tabs and stacks](#tabs-and-stacks)
- [Time, motion, graphics, and platform facts](#time-motion-graphics-and-platform-facts)
- [Inspection and testing](#inspection-and-testing)
- [Repair common mistakes](#repair-common-mistakes)
- [Completion criteria](#completion-criteria)

## Orient and choose the right layer

1. Identify the app, its root `app.contract`, `app.json`, data module, imports,
   authored tests, and requested target hosts. An external app uses `EXACT_APP_DIR`
   with the same exact2 build/driver scripts.
2. Read the nearest working example for the feature. Search `contract/corpus/`,
   `contract/cli/tests/it/`, and the app's sources before designing new syntax.
3. Decide whether the change belongs in Contract, the app's data module, its
   optional native/GPU module, or the host. Do not extend the compiler to avoid
   writing an ordinary data-source function.
4. Check the current branch and dirty files. Preserve other work. Do not commit
   unrelated staged paths or use `git stash` in this repository.

| Work | Normal home |
| --- | --- |
| View structure, UI state, event binding | `.contract` |
| Derived labels, record copies, bounded list transforms, a presentation order by one key (`sortBy`) | Contract expressions / `fn` |
| Network, authentication, storage, a stored or locale order, domain algorithms | App TypeScript/Rust data module |
| Device facts | Reserved source with an admitted shape |
| A system control (button, list, switch, picker, menu, tabs, bars) | Contract's native form ([below](#views-layout-and-interaction)); an app native module only where none exists |
| What only the platform's own object can do (a gesture recognizer, a bar's look, drawing into a view) | An access hatch: `hatch="word"` on the node, native code handed its view ([reference](reference.md#access-hatches)); `bun exact.mjs hatch <word>` writes the stubs |
| Canvas 2D drawing | Data module's canvas surface |
| GPU scene or game | Optional GPU/game artifact |
| App identity, grants/deploy selection, module placement | App manifest and data-module declarations |

For the implementation, the relevant authorities are:

| Question | Source |
| --- | --- |
| Tokens, indentation, escapes | [`lexer.rs`](../contract/syntax/src/lexer.rs) |
| Declarations, statements, elements | [`parser.rs`](../contract/syntax/src/parser.rs) |
| Precedence, templates, callbacks | [`parser/expr.rs`](../contract/syntax/src/parser/expr.rs) |
| Records, locals, callbacks | [`types/records.rs`](../contract/types/src/records.rs), [`types/actions.rs`](../contract/types/src/actions.rs), [`types/lists.rs`](../contract/types/src/lists.rs) |
| Tags, aliases, authored CSS names | [`lower/tags.rs`](../contract/lower/src/tags.rs) |
| Kernel property declarations | [`schema.json`](../kernel/tables/schema.json) |
| Function signatures | [`format.json`](../plan/tables/format.json), field `stdlib`, plus compiler intrinsics |
| Commands and argument checks | [`types/checks.rs`](../contract/types/src/checks.rs) |
| Event payloads and arities | [`analyze/lib.rs`](../contract/analyze/src/lib.rs) |
| Imports and file boundaries | [`cli/sources.rs`](../contract/cli/src/sources.rs) |
| CLI flags and output | [`cli/main.rs`](../contract/cli/src/main.rs), [`cli/build.rs`](../contract/cli/src/build.rs), [`cli/lib.rs`](../contract/cli/src/lib.rs) (diagnostic JSON) |

The useful design context is [LLP 1006](../llp/1006-contract-compiler-v1.spec.md),
[1017.000](../llp/1017.000-contract-v1-1.spec.md), and
[1035.005.000](../llp/1035.005.000-contract-after-the-apps.rfc.md).
A proposal in a design document is not an implemented grammar production.

## The implementation loop

In an app made by `exact new`, run its own `exact.mjs` from the app's directory.
`bun exact.mjs update` regenerates that file, so the app's own verbs go in
`app.json`'s `commands` (`"seed": ["bun", "seed.mjs"]` is `bun exact.mjs seed
web`, run in the app's directory), which it reads and update leaves alone.
Its `contract` verb is exact2's compiler, with paths relative to where you run it:

```sh
bun exact.mjs contract types app.contract -o app.contract.d.ts   # what app.ts imports
bun exact.mjs contract build app.contract --json
bun exact.mjs contract symbols app.contract
bun exact.mjs contract fmt --stdout app.contract
bun exact.mjs contract vocab padding                              # is it accepted, and how
bun exact.mjs web
```

Inside the exact2 checkout, for its own apps, the same commands are
`cargo run -q -p contract -- <command> apps/<name>/app.contract …` and
`bun host/web/dev.mjs --app <name>`. Run `bun install --frozen-lockfile` there once.

Use `build --json` before and after the edit. It returns one JSON array with all
independent diagnostics it can collect, capped at 20; success is `[]`. It is the
whole output: stderr stays empty (but for a note while the compiler first
builds), and `bun exact.mjs contract build`
appends `app.ts`'s type errors (`typescript-TS2322`, …) to the same array. Keep the
exit status: 0 success, 1 compilation/I/O failure, 2 invalid invocation. Read all
diagnostics, including related locations, before making the next repair. Each has
a `severity`: `"error"` refuses the build; `"warning"` (after the errors, also on a
success) builds and runs but is not what you meant, such as a header node the iOS
bar hides (`lower-header-unplaced`), a route's `header` a box below the route,
which iOS leaves in the page (`lower-header-nested`), a heading header at the head of
an app with no navigation route (`lower-header-routeless`: a lone `navigationKey`
is only a root; add a navigation root holding
a separately keyed screen whose first child is the header, or give the header
`appearance="none"` to keep it drawn as written on iOS), a content heading that says the bar's title
again (`lower-title-twice`), a tab bar of one tab (`lower-single-tab`), a
`button` with no words and no `aria-label` (`lower-button-unlabelled`), a dialog's
role on a box (`lower-dialog-role`), an `aria-level` with no `role="heading"`
(`lower-level-without-heading`), a content-box `width="100%"` with padding, wider than
its parent by the padding (`lower-width-padding`), a box whose fixed width ceiling cannot grow
with its text at accessibility sizes (`lower-text-px-width`; use `rem` for the width and its bounds), a bare
`line-height=28`, which is 28 times the font size, not 28 points (`lower-line-height-multiplier`),
screens swapped by an `if` in an app with no `routes` (`lower-screen-by-state`), a
`replace` to another route, pressed on the screen a launch at `/` opens before
anything else changes the router, which the router refuses (`lower-replace-root`), a count before a plural noun, `${n} cards`
(`lower-count-plural`), a row of the app's records that searches another table of them,
`users.filter(u => u.id == p.userId)` in `for p in posts` (`lower-row-lookup`), a
symbol by an alias, not its role (`lower-symbol-alias`), a
`performanceNow()` time sent to a source (`type-performance-now-persisted`), the
launch's `time.epochAtZero` or `time.seed` as a resource's identifying argument, or a
value equal to one at launch (`time.epochAtZero + tick * 60000` while `tick` starts at
0), which keeps its kept answer from ever showing at launch: pass it after `with`
(`lower-resource-launch-identity`), a
state read after its own action assigned it (`analyze-read-after-assign`), a
literal colour that stays the same in dark mode (`lower-colour-one-scheme`, every
site past the cap), a `list` holding a `section` with no `appearance`, which is neither
a grouped list nor a list of cells (`lower-list-section`), and a
name or key a host would refuse or ignore at run time: a
swipe row's `swipeTrailing` naming a button outside its `scroll` (`lower-swipe-wiring`),
a navigation root's `navigationKey` that never meets its routes' (`lower-navigation-key`),
a `navigationSplit` off the root route (`lower-navigation-split`), an `image` source
that is a bare word, neither a symbol nor a file (`lower-image-source`), and a literal `RoundButton`
label of more than the 7 characters its circle holds (`lower-round-button-label`); each names its repair, and fixing it is part of the edit.

Formatting is explicit. `fmt --stdout` previews, `fmt --check` checks, and plain
`fmt` writes. Formatting changes spacing and breaks only: a result that would
parse to a different program is refused (`fmt-tree-change`) and nothing is
written. Avoid formatting unrelated files. `symbols` reports definitions
and references, with component interfaces and inferred action effects. Search
by exact name with `symbols file.contract --name name`.

After compiling, build and drive the actual app. From an `exact new` app:

```sh
bun exact.mjs test web                          # the app's *.test.contract; builds a stale web app first
bun exact.mjs agent web tree "tap add" state logs "screenshot out.png"
bun exact.mjs agent web --size 390x844 "screenshot phone.png"   # a phone-sized viewport
```

The web carrier opens at 420×900; `--size <w>x<h>` (before the operations) opens
another viewport, and a test's first step `size <w>x<h>` does the same for that test.
`resize <w>x<h>`, an operation and a test step, resizes it mid-drive as a person
dragging the window's edge would: the browser's viewport, a macOS window, the
Linux presenter (the driver's op also takes `resize <w> <h>`; a test step takes only
`<w>x<h>`). iOS refuses a mid-drive `resize`; a size given at launch (`--size`, a
test's `size` line) is laid out at that viewport with no safe-area insets, scaled to fit
the app's frame (the safe area, unless the root covers the whole screen).
`close`, an operation and a test step, presses the window's close button as ⌘W
or the red button would (the agent's window is never key, so a typed ⌘W reaches
nothing): the window asks its `beforeunload` first, and the reply says
`closed: true`, or `closed: false` for a window a handler kept (the web's
"Leave site?" is answered "Stay"). macOS and the web; iOS and Linux refuse it.
`--storage <name>` gives a drive a named scratch store, kept between drives on
macOS, iOS and Linux until one passes `--reset-storage`, and on web Chrome when the
next drive passes `--keep-storage` (a browser profile served on one port per
name; a Firefox or WebKit drive starts fresh each time, so a name there lasts
for the drive, including its `reload`). A native drive that names none uses the
store `default`, kept the same way, as the app keeps its files from launch to
launch. A drive that compares with a reference (`screenshot … compare`) starts
empty on every host unless it passes `--keep-storage`, so a plain drive after
one can show what an earlier plain drive stored. A web drive without one has no
app storage: an `app:/` file read finds an empty store (code `ENOENT`, as a
fresh install's first launch does), and a write or SQLite request is refused (`storage
is unavailable in agent mode unless the drive names a scratch store (--storage
<name>)`), so an app that keeps its data there does nothing on a write; a chosen document (`doc:/`)
still opens under its grant. That store, and an authored test's own store, includes
a data module's `secret.keep`: files in the named scratch tree on Apple and Linux,
and the page's `localStorage` on the web. Apple native modules use that store's
data, cache and temporary roots too; a fresh drive clears them, and only picked
temporary documents are cleared on each launch. A nameless drive keeps no secrets.
To show what survives a restart on any host, use an authored test's `reload` step (below).

The driver prints each reply in a short text form. A `tap` or `type` is one line
(`tapped #15 "add" · delivery platform · epoch 2 · clock 0 ms`); a failed one starts
with `ERROR`. `state` prints one line a section (`slots {"count":1}`) and leaves out
the empty ones (null, `[]`, `{}`); one over 8 KB prints its size, its small sections,
the others' sizes, and how to read one (`state resources`, `state feed.0`) or all of it
(`state --full`). `state <name>` reads a state, derive, resource or section of the
record, even where a view carries the same name. Every other reply is one line too
(`clock 600 ms · settled · epoch 5`, `screenshot out.png · 420×900 · …`).
`--json` prints every field of every reply, one JSON object a line.
`--scale <n>` shrinks each screenshot to n pixels a point: an iPhone's are
1206×2622 at its own 3, 402×874 with `--scale 1`, one pixel a point as on the
web, at about a ninth of the pixels. An app's `bun exact.mjs agent ios|macos`
takes `--scale 1` unless told otherwise; ask for more only to check fine pixels.
On a simulator a `window` screenshot's pixels are the display's (`pixels="display"`):
glass, blur and the keyboard as the device draws them. So are a plain one's where glass
or a blur lies in the page; where the display cannot be read (the app not in front, an
overlay) the reply says its materials are approximate, and why.

Inside the exact2 checkout, for Caltrain:

```sh
bun host/web/build.mjs caltrain-web
bun scripts/agent.mjs web tree "tap change-station" "type station-search Palo" state logs
bun scripts/agent.mjs web --test apps/caltrain/app.test.contract
```

For an external app driven from the exact2 root, set `EXACT_APP_DIR` on both build
and drive. A stale
artifact is a failed verification; rebuild what the driver names. A successful
Cargo rlib build does not prove a native app launches or behaves correctly.

tvOS uses the iOS presenter and plan: `bun host/apple/build.mjs --tvos
caltrain-apple --run` builds and launches on an Apple TV simulator. TypeScript
apps provision lean Hermes for `tvos-simulator` once per machine. On a container,
`focusGuide="auto"` makes an entering remote move return to its last focused
descendant, or its first focusable descendant. Other hosts ignore it, including
when the value is bound. The agent driver has no tvOS target yet.

A script drives the same session in JavaScript: `const s = await open({ host:
'web', app, epoch, timeZone, storage, size: [390, 844] })` from `scripts/agent.mjs`
(`size` is `[width, height]`, or the CLI's `'390x844'`), then
`s.tap(target, opts)`, `s.type(target, text | { key })`, `s.clock(arg)`,
`s.tree()`, `s.state()`, `s.logs()`, `s.layout()`, `s.screenshot(path)`,
`s.resize(width, height)`, `s.prefer({ … })`, `s.perf(target)` and `s.close()` — the
CLI's operations by the same names. Their replies are the CLI's `--json` output: `s.tree()`
is `{ roots, nodes: [{ id, type, depth, props: { testId, text, value, … }, children }] }`,
every node in one preorder list (`depth` and `children` give the nesting), and `s.state()` is `{ slots, derives, resources,
pending, … }`. `app` is the app's name; an app outside the exact2
checkout is found through `EXACT_APP_DIR` (its directory), as its own `exact.mjs agent`
sets it, so a script run with plain `bun` sets it to the app's directory too
(`EXACT_APP_DIR=/path/to/app bun ../verify.mjs`, the script kept outside the app
folder, where it would count as a build input); the working directory alone does
not select the app, and `webDist` alone does not select an app. `s.op(request)` is the
host's wire beneath them: it addresses views by numeric `id`, and it refuses a
request it would answer by doing nothing (a `target`, an unknown op, a web
`tap` with no browser input behind it). A reload or a raw browser step goes
through `s.carrier` (`reset({keep: true})` reloads the current browser route with
its store and agent launch facts; `evaluate`, and on Chrome `call`, drive raw browser steps;
`evaluate` takes an expression string or a function of no arguments, which runs in the page,
so it sees none of the script's variables).

Use the existing five repository checks for repository changes. Do not add a
new global check or fixture framework for an ordinary app edit. For documentation,
compile complete examples, parse authored tests, and check local links.

## Language inventory

| Form | Placement | Meaning |
| --- | --- | --- |
| `use A, B as C from "./file.contract"` | File | Names from another file; nothing unnamed comes along (LLP 1091); `contract fmt --uses app.contract` writes the lines `contract-use-missing` names |
| `use Card from "@acme/ui"` / `use Activity from "exact:motion"` | File | A package's names (from `node_modules`), or a built-in's: `exact:motion`, `exact:content` ([content components](#content-components-exactcontent)) |
| `shape Name` | File | Finite record with typed fields |
| `fn name(arg: T): U = expr` | File | Effect-free, nonrecursive expression function |
| `style Name` | File | Literal style attributes |
| `keyframes Name` | File | Constant animation frames |
| `font "Family" = "path.ttf"` | File | Bundled family; block form supports faces |
| `routes nav` | Root file | Route table and implicit router state |
| `component Name` | File | Root (first component) or reusable child |
| `test "description"` | Test file, normally | Agent interaction steps; not a plan assertion block |
| `props` / `inject` | Component | Typed interface / inherited bindings |
| `provide` | Component | Whole-view provider bindings, one per line |
| `slot` | Component | Accept caller-authored children |
| `state name = expr` / `state name: T = expr` | Component | Stored state; its type is inferred unless declared |
| `derive name = expr` | Component | Dependency-driven computed value |
| `resource name = source(args) as shape T` | Root component | Reactive data request |
| `mutation name as shape T [queue]` | Root component | Optional reply slot for explicit sends |
| `action name(args)` | Component | Event transaction; effects inferred |
| `task name mount` / `task name when cond [key=expr]` | Root component | One timer/frame schedule, always or while `cond` holds |
| `view` | Component | UI tree |

Top-level declarations begin in column 1. Indent with spaces. Comments use `//`.
Use double quotes for strings and backticks for templates. Names admit CSS
hyphens, so write `a - b` for subtraction between identifiers. Units are quoted
(`"12px"`, `"50%"`), not number suffixes. Element attribute continuation lines
are deeper than the element and begin with `name=`. Parentheses permit multiline
calls and expressions without creating an indentation block.

Only sixteen words are refused as names, and only where a name is bound (a
state, prop, parameter, `let`, `for` item, shape name, …): `when`, `if`, `else`,
`for`, `in`, `match`, `case`, `as`, `fn`, `and`, `or`, `not`, `true`, `false`,
`none`, `some`. Every other keyword (`key`, `state`, `from`, `refresh`, `view`, …)
is an ordinary name there, so `action searchKey(key: string)` and `action refresh`
compile. Shape fields, named arguments and members take any word
([grammar](contract-grammar.md#lexical-rules)).

`contract vocab` prints the card, what an app writes in one read (at most 12 KB: every
function, the tags with their attributes and events, each part's call, the test steps,
the agent's operations, and how to do a second screen, a sheet, a timer, colour, data, a
test; `docs/card.md` is the same text, checked in); `contract vocab --all` lists every built-in tag, attribute, CSS
property and standard function the compiler accepts, with each property's value kind and
default and each function's signature; `contract vocab <name>` answers for one (a function's rules and
an example too: `contract vocab formatDate`), or says what to write instead
(`contract vocab sort`). Check there before guessing at CSS or a function.

## Values, expressions, and functions

An expression Contract accepts means what the same TypeScript means, except as
this table says (LLP 1116.016 D1; `semantics/corpus/typescript/` checks each
against JavaScript), and a TypeScript form Contract refuses is refused with
what to write. Nothing runs because a value changed (no `useEffect` or watch).

| Contract | Because |
| --- | --- |
| No truthiness: a condition is a `bool` (`if name != ""`) | every condition says what it tests |
| Absence is an option (`none`, `some(x)`, `match`), not `undefined` or `null`; no `??` or `?.` | a value whose type has no absence is never absent |
| A value never changes: no `push`, `sort`, `xs[i] = …`; `concat`, a record copy | an action is one transaction |
| `==` compares lists and records by value; no `===` | values have no identity |
| A string is well-formed: a lone surrogate becomes U+FFFD (`"😀".slice(0, 1)`) | every host holds Unicode scalar values |
| A stored number is finite: a commit that would put `NaN` or `Infinity` in a state or derive is refused, and the write that caused it does not land | every host can carry it |
| `toFixed` and the format functions print `""` for a non-finite number | one rule for every format |
| Names hold hyphens, so subtraction takes spaces (`a - b`) | CSS's names |
| A binding's list work is bounded (65,536 steps); a row's own binding has its own bound, so a row that searches another table costs rows × items (`lower-row-lookup`) | determinism on every host |
| No comparator sort: `sortBy(xs, x => key)` orders by one number or string key, stably (`"desc"` third; a second key is an earlier `sortBy`). Grouping, joining, aggregates and a stored or locale order are the data module's; the view names an order it relies on where it reads it (`loadTasksByDueDate()`) | a view computes what it shows, the same on every host |

- Types are `number`, `string`, `bool`, declared shapes, `option<T>`, `T[]`,
  and `action` for behavior interfaces. Do not emit authored `any` or object types.
- `[a, b, c]` is a list of its items, which may span lines and keep a trailing
  comma; the items have one type, met as a ternary's arms are (`[none,
  some(1)]` is an `option<number>[]`; `[1, "a"]` is `type-list-item`).
  `xs.concat(ys)`, `xs.slice(start, end?)`, `xs.includes(x)` and
  `xs.indexOf(x)` are the web's array methods: `[...xs, x]` is
  `xs.concat([x])`, `includes` finds a string, number or bool (by
  SameValueZero, as the web does) and `indexOf` says where (by `===`: `NaN` is
  never found, `-1` for none). Over text, `s.indexOf(t)` is a position in
  UTF-16 code units, and `s.split(", ")` cuts text into a `string[]`
  (recipient chips). A list the
  screen keeps for the session — a selection, open or collapsed ids, per-row
  offsets — is `state` built this way; a list the app keeps across launches, or
  a server owns, belongs to the data module, and so do grouping, aggregates
  and a stored order (a view orders what it holds with `sortBy`):

  ```contract
  state collapsed = []
  action toggle(id: string)
    collapsed = collapsed.includes(id) ? collapsed.filter(c => c != id) : collapsed.concat([id])
  ```
- `none` and `[]` need an inferable element type. A state initialized by either
  usually gets that information from later assignments; a typed argument or
  the other conditional/match arm can also supply it, and a state may declare
  it: `state picked: option<Item> = none`, `state ids: string[] = []`.
- An option directly inside an option is refused (`type-option-option`):
  `option<option<T>>` written, `some(none)`, `first` or `at` of a
  `option<T>[]`, a mutation `as shape option<T>`. The web erases `some`,
  so `some(none)` would be `none` there; hold the inner option in a record
  field. A type nests at most 64 deep, through shapes, lists and options
  (`type-too-deep`).
- One evaluation (an action body, a derive, a binding, a key, an argument)
  takes at most 65,536 list steps (each `map`/`filter` body run, each item
  `join` prints or `concat`/`slice`/`split` keep, each item `includes` or
  `indexOf` scans), builds strings of at most 64 MiB of UTF-8, and values of at
  most 2²⁴ values and 64 MiB of string bytes. Every target refuses the same
  step with the same reason: an action is refused with nothing changed, a view
  binding stops the runner (LLP 1090).
- A state's initializer runs before any resource answers and before any derive:
  it reads props, injects and the states declared above it, nothing else
  (`type-initializer-scope`). Derive a value from a resource instead, or keep
  per-row state in a component used inside `for`.
- `Shape(field=value, …)` constructs every field exactly once.
  `Shape(base, field=value, …)` copies a base of the same shape and replaces fields.
  The one positional base comes first. Compiler-owned shapes are not constructible.
- Use field access, arithmetic, comparisons, boolean operators, ternaries, and
  exhaustive option matches. There is no truthiness or optional chaining.
- Inline match is `match x { case some(v) => a, case none => b }`; the comma is
  required and the `some` arm comes first. The block statement/view form has
  separate indented `case` arms.
- `xs.map((x, i) => expr)` and `xs.filter(x => bool)` return values, not nodes.
  `xs.join(separator)` accepts primitive items. `first(xs)` and `xs.at(i)`
  return options; `at` supports negative indices.
- Two strings compare with `<`, `<=`, `>`, `>=` in UTF-16 code-unit order, as on
  the web (`end > start` for `"HH:MM"` times). `s.slice(0, -1)`,
  `s.replaceAll(find, with)` and `s.toLowerCase()` are the web's string methods.
- Numbers have `floor`, `ceil`, `round` (JavaScript's `Math.round`: `round(-2.5)` is
  -2), `min`, `max`, `%`, `formatNumber` (`"compact"` is `1.2K`, `"decimal"` `1,234.5`,
  `"percent"` `26%`, `"currency", "USD"` `$1,481.47`), `toFixed` and `formatDecimal`. A
  field's text is a number through `match parseNumber(s) { case some(n) => …, case
  none => … }`: a decimal numeral, trimmed, or `none` (`"12px"`, `""`).
- Money is a count of cents printed with `formatDecimal(cents, 2)`, which is
  exact (`1234` is `"12.34"`, `-5` is `"-0.05"`). A price held as dollars
  becomes cents with `round(price * 100)`, which is right for a price of at
  most two decimals under a trillion (`19.99` is `1999`, though `19.99 * 100`
  is `1998.9999999999998`); past about `3.5e13` the double cannot hold the
  cents, and a half cent such as `1.005` has no exact binary value and
  becomes `100`, so keep money in cents from the source when amounts can
  have more places (tax, a split bill). `formatDecimal` of a number that is
  not an integer prints `""`, so round first. `x.toFixed(2)` is the web's
  `x.toFixed(2)`, for a measured number (`km.toFixed(1)`): it rounds the
  binary value too (`(1.005).toFixed(2)` is `"1.00"`) and sums of dollars drift
  (`0.1 + 0.2`), which is why money is counted in cents. Both take the digits as a whole-number literal (`toFixed` 0–100,
  `formatDecimal` 0–20), never a variable, and print `""` for `NaN` or
  `Infinity` (JavaScript's `toFixed` prints `"NaN"`).

```contract
component Cart
  state cents = 1999
  state km = 12.34
  view
    column
      text `Total $${formatDecimal(cents, 2)}` testId="total"
      text `${km.toFixed(1)} km` testId="distance"
```
- Dates: `formatDate(ms, offset, "iso")` is `YYYY-MM-DD`, and `calendarDiff(from, to,
  "years")` (or `"months"`) is the whole periods between two such dates as an
  `option<number>`, counted as an age is (a Feb 29 birthday has its year on Mar 1).
  `formatDay(then, now, offset, "day")` names a day from `now` (`Today`, `Yesterday`, else
  the date; `"day-time"` adds the time, `"list"` is a list row's, as Mail's).
- There is no general list append: add an item to
  resource-backed data in its source and answer the updated list (a mutation that
  `refreshes` the list's resource, or its own answer).
- The web's string and array methods are written as methods, with their names
  and meanings: `s.trim()`, `s.includes(q)`, `xs.filter(x => …)`, `n.toFixed(2)`,
  `xs.length`. The free spelling (`trim(s)`) is refused with the edit
  (`syntax-method-form`), and so is a method Contract lacks (`xs.reduce(…)`,
  `syntax-method-call`). The rest of the roster is functions: `first(xs)`,
  `round(n)`, `parseNumber(s)`, `formatNumber(…)`. There are no object
  literals, spreads, general lambdas, array indexing,
  assignment expressions, or JavaScript built-ins by implication.
- `fn label(done: bool): string = done ? "Done" : "Open"` is a function: parameters
  and the return type are explicit, after `:` (not `->`). Its body is one expression over
  its parameters and standard calls (including `performanceNow()`), without component-state
  capture or recursion. Pass an app value in; do not invent an ambient reference.
  A `fn` named like a standard function (`fn indexOf`) shadows it in every
  free call of the app, so a standard function added later never breaks an
  app that had the name first; an action or a host command keeps its own rule.
  A method call (`xs.indexOf(x)`) is always the built-in: calling one whose name
  the file also declares is refused (`syntax-method-shadowed`).
- Named arguments belong to component uses, record constructors,
  `t("key", placeholder=value)`, `empty(field=value)`, canvas `surface=` bindings,
  and the commands `share(…)`, `showNotification(…)`, `scrollIntoView(…)`, `playSound(…)` and
  `stopSounds(…)`. Every other function takes
  positional arguments.

For the full roster and special calls, see
[standard functions and intrinsics](contract-grammar.md#standard-functions-and-intrinsics).
For record behavior in a callback, use [records](../contract/corpus/records.contract)
and [the record tests](../contract/cli/tests/it/records.rs).

## State and action semantics

Action reads see the starting state. Writes commit together. They do not provide
imperative read-after-write visibility. Bind intermediate calculations with `let`:

```contract
component App
  state count = 0
  state doubled = 0
  action advance
    let next = count + 1
    count = next
    doubled = next * 2
  view
    button press=advance testId="advance"
      text `${count}/${doubled}` testId="result"
```

The first press yields `1/2`. Writing `doubled = count * 2` instead would use the
old count and yield `1/0`. The same snapshot rule applies inside nested branches
and to locals declared after a state assignment. The compiler warns of such a read
(`analyze-read-after-assign`), naming the line that assigned the slot.

A local is immutable, block-scoped, and visible only after its declaration. It
cannot shadow an in-scope name. Two disjoint arms may declare the same local name.
An action can assign only its own writable state/mutation slots, never a prop,
derive, resource value, or arbitrary record field. Replace a record with a copied
record. `send` targets a mutation owned by that component.

Permitted statements: assignment, `let`, `send`, `refresh`, known host command,
a call of an action, `if`/`else` (`else if c` is `else` around one nested `if`),
and option `match`. No loops. The compiler
infers effects from the body, through its calls; `writes` is a refusal, not an
optional annotation. Use `symbols` when you need the inferred write set.

An action calls an action of its own component, an `action` prop or an injected
action by name with its parentheses (`play()`, `arrive(path)`; a bare `play` is not a
statement), anywhere a statement goes (LLP 1089). The call is
the callee's statements run where it stands, in the same commit: one rollback,
and the callee reads the state the action started with, as every statement does.
A name that is a host command stays the command. Never give an action, an `action`
prop or an inject a host command's name (`close`, `reload`, `setScheme`, `focus`, …)
if you will call it: the call is refused, `syntax-call-ambiguous`, so a host command
added later can't silently take it over. Binding such a name is allowed. A call gives no value; compute
values with `fn`. Write a repeated sequence once and call it:

```contract
component App
  state location = "/"
  state query = ""
  state sel = 0
  action arrive(path: string)
    location = path
    query = ""
    sel = 0
  action openItem(path: string)
    arrive(path)
  action goHome
    arrive("/")
  view
    column
      button press=openItem("/docs") testId="open"
        text location testId="where"
      button press=goHome testId="home"
        text "home"
```

Because the callee sees the starting state, a slot its caller assigned first
would be stale behind the call, and the compiler refuses that read
(`analyze-call-stale-read`): `sel = next` then `follow()`, where `follow` reads
`sel`. Pass the value it should see instead: `follow(next)` for the new one, or
a `let` bound before the assignment for the old one. Two calls that each read and
write one slot (`move(1, 0)` then `move(0, 1)`) are refused the same way: both
read the starting cell, and the last write wins. Calls in exclusive branches
(`if k == "ArrowUp" …` then `if k == "ArrowDown" …`) are separate paths.

A derive is not mutable storage, an async effect, or a timer. Derive cycles are
refused. Avoid unnecessary state that can be calculated from existing values.

### A remembered setting: `persist`

A root `state` that ends in `persist` is kept across launches (LLP 1116 D5), as
SwiftUI's `@AppStorage` or a React `useState` over `localStorage`:

```contract
component App
  state tipPercent = 18 persist
  state units = "C" persist
  action toggleUnits
    units = units == "C" ? "F" : "C"
  view
    button press=toggleUnits testId="units"
      text `${tipPercent}% · °${units}`
```

- Before the first frame the store is read: a kept value of the state's type
  replaces the initializer's. A missing one keeps the initializer's, and so does
  one of another shape (the state's type changed since), which `logs` names
  (`persisted state units: the stored value does not fit string: …`).
- After every commit that stands, the store holds the state's value: the page's
  `localStorage` (`exact.state.<name>`), the app's `UserDefaults` domain on Apple
  (never the Keychain), a file under the app's data directory on Linux. The key
  is the state's name, so renaming the state forgets what was kept.
- Small settings only: a number, string or bool, or an option or list of one, at
  most 16 KB as JSON (a longer value is not kept, and `logs` says so). `persist`
  on a record, a list of records or a state holding an action is
  `type-persist-type`: keep app data in a source. A child component's state is
  `type-persist-child`; a derive, `syntax-persist-derive`.
- Each authored test starts from an empty store, and its `reload` reads back what
  it kept. A drive with `--storage <name>` keeps persisted state in that store
  between drives (on the web, the next one passes `--keep-storage`; on a native host
  `--reset-storage` empties it); a drive without one starts from the declared values and keeps
  nothing past the drive. The recipe:
  [a remembered setting](recipes/persisted-setting.md).

### Editing a value: the field's contract

An `input` handler should update raw draft state on every edit so live search,
validation and button enablement follow typing. A text field fires `change` on
Enter or blur; its handler should normalize the draft. Use `submit` for explicit
submission.

A text field is re-set only when what its `value` binding reads changes, never
after each keystroke (React writes the bound value back; Exact does not, so a
half-typed `-` or `1.` survives). That makes the contract:

- **while editing**, the field is bound to raw text in state that `input`
  always writes;
- **validation** reads the parsed value (`parseNumber`, a trim, a length) and
  shows a hint, without touching the text;
- **on commit** (`change`, which a text field fires on Enter and on blur), the
  action writes the accepted value and writes the normalized text back into the
  draft, which changes the binding and redraws the field.
  Since `change` also fires when an edited field loses focus, it is no
  "submit on Return": a form submitting on it submits on a tap away from the
  field (an eye, a "Forgot password?"). Submit on Return with `submit`, which
  runs on Return alone, after the commit (and never for an IME's Return).

A field bound straight to the accepted value breaks this: an action that
normalizes `-2` to the `0` it already held leaves the binding unchanged, so the
field keeps showing `-2`. So does normalizing while the person types: a `task … when draft != …`, a
timer or an `input` action that rewrites the draft (an empty field back to
`"1"`) puts text back under the caret mid-edit, and the next keystroke lands
after it (`1` then `3` reads `13`). Normalize only in `change`.

```contract
component Quantity
  state count = 1
  state draft = "1"
  action edit(text: string)
    draft = text
  action commit(text: string)
    match parseNumber(text)
      case some(n)
        count = max(0, round(n))
        draft = `${max(0, round(n))}`
      case none
        draft = `${count}`
  view
    column gap=8
      input value=draft input=edit change=commit inputmode="numeric" aria-label="Quantity" testId="qty"
      text (match parseNumber(draft) { case some(n) => (n < 0 ? "Must be 0 or more" : ""), case none => "Enter a number" }) testId="qty-hint"
      text `Ordered: ${count}` testId="qty-count"
```

`type "qty" "-2"` leaves `-2` in the field with the hint; `type "qty" key
"Enter"` commits, and the field reads `0`. A checkbox bound to a resource's
field follows the same rule from the other side: it shows the resource's value,
so it snaps back until the save answers and the resource is read again (LLP 1102
§3.16).

## Composition and lifetime

The first component is the root. Each component use is `Name(prop=value, …)`.
Supply every declared prop exactly once; props have no defaults (pass `none` for
an option). An `action` prop can receive a reference with
captured arguments; the eventual event payload is appended at invocation.

A prop or inject named like an app `fn` is refused when an expression in the
component calls that name and inlining could substitute the prop into the call.
Rename either declaration. Handler-only uses are allowed. An expression's
`match` or arrow binder protects its bound name; a view's `for` or `match`
binder does not protect call heads.

A derive supplies a value, even when its expression is a function call.
Beside `fn pick`, `derive pick = pick(1)` makes `${pick}` read that result;
`pick(5)` still calls the function.
States and device-fact resources also supply values only: beside `fn pick`,
`state pick = 7` makes `${pick}` read `7`, while `pick(5)` calls the function.
These names leave calls intact in derives, state initializers, actions and slot fills.

A child's action may call its `action` props and injected actions, with the
arguments after those captured where they were bound: the parent's action runs
in the same commit, and the child never writes the parent's state itself. A
swipe that decides, in the child, to tell its parent resets its own state, then
calls the prop last in an `if` arm:

```text
component Row
  props
    id: string
    archive: action
  state dx = 0
  action release(dy: number)
    dx = 0
    if dy > 120
      archive(id)
```

Children may hold state, derives, and actions. They cannot declare resources,
mutations, or tasks. Lift shared data requests to the root and pass values and
actions. A child used under `for` owns row state keyed to that row's identity.
Stable keys matter when content is inserted, removed, filtered, or reordered.

A root that has outgrown the 1,500-line cap keeps its members and moves them
into files by feature: `members "./composer.contract"` in its body, and in that
file a `members App` block of states, derives, resources, mutations, actions,
tasks and sequences. The program is the one-file program, and every file sees
every member; each file `use`s the shapes and `fn`s it calls
(`docs/contract-grammar.md`, "Components and actions").

A state cannot start from a resource, but a child's state can start from a prop.
So a form that edits a saved record is a child made once the record is in: the
source answers `loaded: true`, the placeholder says `false`, and the child's
states take the record's fields when it is made. The initializer runs once per
child: a later `draft` (a refresh, a normalized answer) does not reset the form,
and while a refresh is out `saved` keeps its value, so the editor stays.

A native host (and the wasm web target; the default JS target keeps none) can make
that child from a kept answer. When the data module is not ready at boot, a resource
whose source read device state (a file, a database, a secret, a watched topic) shows
the last answer the runner kept for it (asked with the same identifying
arguments, and at most 256 KiB counting them, 512 KiB for all of them together; `with` values are not counted), so
the form's states take those fields. A larger answer is not kept, and an older one
kept for the resource is forgotten, so a launch never shows an answer since replaced;
`logs` says `too big to keep` once, naming the resource and what to do. An
identifying argument that changes every launch (the launch's time or seed) never
matches the kept one: pass it after `with`, as `resource lib = library() with
time.epochAtZero`; the compiler and `logs` both say so.
Keep a first screen's resources small and move their bulk into a resource the first
frame does not need. A write the resource does not hear about (a file or database save from
a mutation without `refreshes saved`, not a `store` write it reads) leaves that
answer behind: after a restart the form opens with the old fields and keeps them
when the fresh answer lands. Refresh the resource after each write, as below, or key
the child by a string or number from the answer (`for d in [saved] key=…`) so a
different answer makes it again.

```contract
shape Draft
  name: string
  loaded: bool

component App
  resource saved = loadDraft() as shape Draft else empty(name="", loaded=false)
  mutation stored as shape Draft refreshes saved
  action keep(name: string)
    send stored = saveDraft(name)
  view
    main
      if saved.loaded
        Editor(draft=saved, keep=keep)

component Editor
  props
    draft: Draft
    keep: action
  state name = draft.name
  action edit(value: string)
    name = value
  action save
    keep(name)
  view
    column
      input value=name input=edit aria-label="Name" testId="name"
      button "Save" press=save testId="save"
```

A component `provide` section lists `name = expr` or bare `name` entries. A
descendant declares `inject` with typed fields. The nearest provider in the
component-use chain wins and covers the providing component's whole view.
The old view wrapper `provide name = expr` is refused. Use a small wrapper
component if a subsection needs different values.

`slot` declares a receiving component's one content slot. `children` places the
caller's nodes in its view. The fill keeps its caller's variables and providers,
not the receiving component's provider override. Do not invent named slots or
render-function props.

Imports resolve local `.contract` declarations, with cycle and app-boundary
checks. Paths begin with `./` and cannot contain `..` segments. They do not load app logic or expand the available built-in function
roster. Compiler-owned router names exist only with a `routes` declaration.

### Tabular numbers

Use `font-variant-numeric="tabular-nums"` for **numeric columns** and **values that
change in place**: prices in a ledger, timetable times, timers, scores, counters,
progress percentages and recalculated totals.

Use default spacing for prose, ordinary labels, static standalone statistics,
and fields merely accepting numbers. Choose by how the value is used, regardless
of its size or currency symbol: static columns need alignment, and updating
headlines need stable digits.

```contract
component App
  state balance = "$1,234.50"
  view
    text balance font-variant-numeric="tabular-nums"
```

Apply it to numeric text, keeping the platform font. It makes digits equal-width
without monospacing letters. Font support is required; unsupported fonts remain
unchanged. The property inherits. `normal` restores the font's default, which may
already be tabular; Exact does not accept `proportional-nums`.

Equal-width digits do not fix the whole value's width: `99` → `100`, signs,
precision, separators and units can still move content. Reserve space for the
formatted range when layout must stay still. Align numeric columns to a common
edge (usually right for left-to-right numbers), with consistent units and precision
where appropriate. Digits alone do not align decimals; do not pad with spaces or
misleading zeroes.

Supported for ordinary text on macOS, iOS, tvOS, Linux, Windows, Android and both
web targets, including prerendered pages. Exceptions:

- **Terminal:** ordinary digits already occupy equal-width cells. Explicit
  `font-variant-numeric` settings are currently rejected.
- **Android editable fields:** unsupported on native `input` and `textarea`;
  they keep their font's default digits.
- **Native buttons:** unsupported. Keep their typography; use separate `text`
  for a readout. This property also does not style Canvas/GPU drawing or native hatches.
- **Text wrapping around shapes on web:** a paragraph using `tabular-nums` skips
  Exact's wrapping along image or shape outlines and uses ordinary browser layout.
  Keep readouts separate from prose that needs this wrapping.

`exact:content` components set their own typography: `SummaryCard` uses tabular
digits for `value` and `detail`. Use `text` for a static statistic needing default
spacing.

Check the actual font, locale and host, including fallback digits, `11` → `88`,
`99` → `100` and large accessibility text. Widths can differ across hosts. Keep
locale-aware formatting and accessible labels. See
[numeric-readout](recipes/numeric-readout.md) for sizing.

### Content components: `exact:content`

The platform answers the controls and the chrome; the app's own content is the
author's to design (LLP 1116.000 D3): a summary at the top, bars where numbers
compare, rows with a hierarchy, an empty state that says what to do.
`exact:content` is a built-in library of nine components that draw it with
nothing but roles (`-exact-secondary-background`, `-exact-secondary-label`,
`AccentColor`, the system hues, the text styles, symbols, the determinate
`progress`), so dark mode, Dynamic Type and the tint come with them:

```contract
use SummaryCard, StatBar, RingProgress, DoneMark, EmptyState, RowText from "exact:content"

shape Day
  key: string
  name: string
  ml: number

fn ml(n: number): string = `${formatNumber(n, "decimal")} ml`

component Water
  state total = 750
  state days = [Day(key="mon", name="Mon", ml=1800), Day(key="tue", name="Tue", ml=2250)]
  action drink
    total = total + 250
  view
    scroll height="100%" accent-color="-exact-system-teal"
      column gap=16 padding=16
        RingProgress(label="of 2,000 ml", value=ml(total), fraction=total / 2000, tint=none)
        SummaryCard(label="Today", value=ml(total), detail="of 2,000 ml", progress=some(total / 2000), symbol=some("symbol:activity"))
        for d in days key=d.key
          StatBar(label=d.name, value=ml(d.ml), fraction=d.ml / 2000, tint=none)
        row justify-content="space-between"
          for d in days key=d.key
            DoneMark(caption=d.name, done=d.ml >= 2000)
        if total == 0
          EmptyState(symbol="symbol:calendar", title="No water yet", message="Each glass you log adds to today.")
            button "Log a glass" press=drink buttonStyle="bordered-prominent"
        for d in days key=d.key
          RowText(title=d.name, detail="", value=some(ml(d.ml)))
```

| Component | Props | Draws |
| --- | --- | --- |
| `SummaryCard` | `label: string`, `value: string`, `detail: string`, `progress: option<number>`, `symbol: option<string>` | a card: the symbol and label, the value large in tabular figures, the detail beside it where the card is wide and under it where it is narrow, the fraction as `progress` |
| `StatBar` | `label: string`, `value: string`, `fraction: number`, `tint: option<string>` | the label and value (the value under the label where both do not fit) over a bar `fraction` (held to 0–1) of the width: the accent short of the goal, the system green at 1 or past it, `tint` (a colour role) when given; one `meter` to a reader |
| `RingProgress` | `label: string`, `value: string`, `fraction: number`, `tint: option<string>` | a ring filled `fraction` of the way round from the top, in the bar's colours over a grey track, the value large in its centre and the label under it, at the default body's sizes at every text size; 200 across, down to 120 in a narrower box; one `progressbar` to a reader |
| `DoneMark` | `caption: string`, `done: bool` | a filled check in the system green, or an empty circle in grey, with `caption` (short: a weekday; `""` for none) under it; one image to a reader, named by its caption and state ("Mon, done") |
| `EmptyState` | `symbol: string`, `title: string`, `message: string`; a slot | a centred symbol (none at the accessibility text sizes), the title as a heading, the message; the nodes indented under the use (your `button`) under them; a `scroll` where it is taller than its space (in a flex column, give the column `min-height=0`) |
| `RowText` | `title: string`, `detail: string`, `value: option<string>` | a title, a secondary line under it (`""` for none) and a trailing value in tabular figures (under them where they do not fit on one line): a grouped list `row`'s content, or any list's |
| `Badge` | `label: string`, `tint: option<string>` | a status in a word ("Full", "Cancelled") in a capsule, the caption style semibold, keeping its own size beside a title; `none` the neutral grey, `some("AccentColor")`, or a system hue under white ink (dark on the light hues: green, mint, teal, cyan, yellow, orange); read as its text |
| `RoundButton` | `label: string`, `tint: option<string>`, `press: action`, `disabled: bool`, `testId: string` | the platform's own button as an 80-point circle (the Clock's Start, Stop, Lap): filled with `tint` (a role: `"AccentColor"`, `"-exact-system-red"`), ink the host picks for contrast; a light hue (green, mint, teal, cyan, yellow, orange) is the platform's tinted style, the hue on the hue at low opacity; `none` is the neutral grey with the label's ink; the title stays the default body size at every text size |
| `AnswerTile` | `label: string`, `status: string`, `press: action`, `testId: string` | a full-width tile for one answer, the label in the headline style; `status` `"idle"` (a neutral fill), `"selected"` (the accent's outline and checkmark), `"correct"` (the system green, dark ink, a check), `"wrong"` (the system red, white ink, a cross), `"dimmed"` (the rest, once told), or `"chosen"` (the person's own pick once told or closed: the selected look, disabled, so a closed poll still marks the vote while the rest dim); told, dimmed and chosen tiles are disabled; as a grouped list's row it is the list's cell on every host (`-exact-row-look`), the checkmark its accessory; a reader hears "Quito, correct" |

- Props have no defaults: pass `none` for an option. A symbol is an `image` source:
  a role (`"symbol:calendar"`) draws on every host, `"symbol:sf/<name>"` on Apple only.
- They are in the app's accent: `accent-color` on the root (or any ancestor) tints
  them as it tints the controls, on the web and iOS (macOS keeps the system accent,
  as AppKit does). A state is a second colour: progress
  short of its goal is the accent, a goal met (`fraction` 1 or more, a card's
  `progress` at 1) the system green, done against not done green against grey. A
  `tint` overrides it: a budget gone over passes `some("-exact-system-red")`, a
  breakdown whose bars are shares rather than goals passes `some("AccentColor")`.
- They draw no chrome, and two controls: `RoundButton` and `AnswerTile`, the
  emphasis a timer's or a quiz's screen needs, which paint on a `button` would
  make your own box. Any other action is your own `button`, beside a card or
  indented under `EmptyState`. Inset-grouped lists are for settings and forms; a
  content screen is a `scroll` of these.
- Each fills its container's width; arrange them with CSS. In a `flex-wrap="wrap"`
  row, `flex="1 1 320px"` puts a card beside the bars on a wide window and stacks
  them on a phone.
- The source is `contract/lib/content.contract`. To change one, copy it into the app.

## Data requests and side effects

Choose the mechanism from its lifetime:

| Requirement | Mechanism |
| --- | --- |
| Value follows query arguments | `resource result = source(args) as shape T` |
| Source needs changing context while the same kept answer is suitable at boot | `resource result = source(identity) with context as shape T` |
| Explicit command with a reply | `mutation reply as shape T`, then `send reply = source(args)` |
| Re-request current resource arguments | `refresh result` |
| Refresh reads around a mutation | `mutation … refreshes resourceA, resourceB` |
| Show a write before its reply (optimistic) | `refreshes` on the mutation, and the source's `overlay` |
| React once to a settled mutation | `mutation … then actionName` |
| Writes that must all land, in order | `mutation … queue`: one in flight, later sends wait their turn |
| Run once after a delay, while a condition holds (a toast, a debounce) | `task … when cond` with `after(ms, action)` |
| Repeat while a condition holds (a game tick, a pulse) | `task … when cond` with `every(ms, action)` |
| Act when a sheet or route opens (scroll the chosen item into view, load its draft) | In the root, even when the sheet is a child component's view: `task reveal when top(nav).name == "icons" key=top(nav).id` with `after(1, reveal)` |
| Pending indicator | `pending(resourceOrMutationName)` |
| Resource request failed without an answer | `failed(resourceOrMutationName)` |
| Why it failed, to branch on or show | `failure(resourceOrMutationName)`: `none`, or `some({ code, message, reason })`, `code` a domain refusal code when supplied, otherwise one of `offline`, `timeout`, `refused`, `shape`, `storage`, `error`; `reason` a Snapback refusal's code (`SLOT_FULL`), else `""` |
| Initial resource fallback | `else empty(field=constant)`, or `else source(values)` answered once at build |

Resources read as their declared type. Mutations read as `option<T>` and start at
`none`, so a mutation's `T` is not itself an option. Do not treat a resource as an optional wrapper unless its declared type
itself is optional. A mutation reply is unwrapped with match.

An answer is held to its shape exactly: a field the shape does not declare, at any
depth, fails the resource as a thrown error does (TypeScript lets a spread such as
`{ ...row }` through). The view keeps its placeholder or last value and
`failed(x)` is true, so a banner on `failed` reads "unreachable" for what is a
shape mismatch: `state` names the field under `failed`, a CLI drive says so on
stderr, and a failing `expect` names it. Project a backend row onto the shape
field by field (`({ id: row.id, title: row.title })`).

`failure(x)` says why, for the view to branch on: `none` while `x` has not
failed, else `some` of a `Failure`, whose `code` is the nonempty domain `reason`, or the infrastructure class the same on every host (`docs/contract-grammar.md`), whose `message`
is the text `state.failed` shows (for a developer; it differs by host), and
whose `reason` is the string the source's error carried as `reason` (a
Snapback refusal's code), else `""`:

```contract
shape Item
  id: string

component Items
  resource items = loadItems() as shape Item[] else empty()
  derive banner = match failure(items) {
    case some(f) => f.code == "offline" ? "You're offline" : f.code == "shape" ? "This app needs an update" : "Couldn't load items",
    case none => ""
  }
  view
    column
      if banner != ""
        text banner testId="banner"
```

`offline`, `timeout`, `refused` and `storage` reach the view only when the
TypeScript module lets the `fetch` or storage rejection through (rethrows it, or
does not catch it): an error it makes of its own, one for an HTTP 500 among
them, is `error`. Its own error keeps its `message`, and `failure(x).message`
is that text. An error with a string `reason` property keeps that too, as
`failure(x).reason`: the snapback4 driver's `refused` throws a server's
refusal with its code there, so a query's `require … else NOT_MANAGER` is
`f.code == "NOT_MANAGER"` in the view. Mutations expose their latest admitted request failure too; a new send, assignment or success clears it. `bun scripts/agent.mjs web "fail fetch https://api…"` fails
those fetches as a lost connection, so the banner says "You're offline".

`with` takes one or more expressions, before `as shape`, and appends them to the
source's arguments. All arguments still trigger re-asks and identify live
requests. Only an eligible persisted answer admitted while the source is unready
at boot matches the call arguments alone; it can then stand while activation is
pending. Without `with`, every argument identifies that kept answer. Keep
account/tenant IDs and representation choices in the call, credentials below
the seam, and use `refresh`/`refreshes` for counters whose only purpose is another
ask. The default web JS target keeps no persisted resource answers
([LLP 1027.005](../llp/1027.005-resource-identity-and-request-context.rfc.md)).

Resource inputs are not asynchronous prerequisites, and HTTP request lanes do
not make resources depend on one another. A resource that reads another resource
uses its current displayed value; it does not wait for the upstream request to
succeed. While that request is pending, the value may be a retained answer, and
`not pending(upstream)` does not prove the latest request succeeded.

An authenticated source must await the app's shared current-session helper
before issuing its protected request. It must verify that the session authorizes
the principal named by the resource call. The helper and auth generation are app
code, not Exact APIs. The app must advance its auth generation and clear its
authorized principal whenever the principal changes or signs out. A request
that must belong to that auth attempt should take the generation as a resource
argument and return the principal and generation as fields in its answer. The
view should show private data only when both fields match the currently
authorized principal and generation. Checking only the owner cannot distinguish
signing into A, then B, then A again. A pending session resource, or one that is
merely not pending, does not prove authorization.

Before committing a refreshed session or shared owner-sensitive cache write,
recheck the generation at the commit point, including inside queued work. When
possible, partition persistent caches by owner so one owner's valid result can
still populate its cache after the view changes. If writes share a slot, guard
the commit by owner and auth generation. Do not suppress a valid per-owner cache
write only because another request became current. See the
[fetch/loading/error recipe](recipes/fetch-loading-error.md) for retained-answer
and owner-gating patterns. The [REST starter](patterns/rest/app.contract) uses
public development identities and does not implement an authenticated session
lifecycle.

The current request owns its answer; older replies cannot overwrite a newer
request. A newer send or an assignment drops a mutation's in-flight reply. It
does not undo storage operations the source has already started; those
operations and any serialized storage chain behind them run to completion.
Other source work, such as a fetch, may be aborted when an answer is replaced
(see the [runtime reference](reference.md#what-a-data-module-can-use)).

The action sending a mutation without `queue` must not also assign its slot to
`none`. Assignments commit after sends, so the reset can discard the new reply
even when it appears first. Keep the mutation for its reply and hide an old error
while `pending(added)`, as the [after-a-write recipe](recipes/after-a-write.md)
does. `none` means the slot contains no reply, not that the write failed: use
`failure(added)` for a request failure, and match a shaped reply's `ok` or error
for a domain refusal.

An action that sends one mutation twice on one path is refused
(`analyze-send-twice`) because the second send would forget the first reply, counting
the sends of the actions it calls. Every `if` is read as one that can run: two
sends are separate paths only as arms of one `if … else if … else` or `match`,
or in `if`s testing one unchanged name against different literals. Send one
combined request, use a mutation per request, or declare the mutation `queue`
([LLP 1092](../llp/1092-sends-that-queue-and-timers-that-wait.rfc.md)). A
`queue` mutation (`mutation wrote as shape Ack queue then afterWrote`) keeps one
request in flight; every later send waits, in order, with the arguments it was
made with, and is asked after the reply before it and that reply's `then`, so
`then` runs once per reply, in send order. `pending(m)` is true while a send is
in flight or waits: send while it is pending, since `not pending(m)` means the
spinner is off, not that a send may be skipped. Assigning a queue's slot forgets
nothing — every reply still lands over it — so a mutation that must drop a late
reply (a session's sign-in) does not declare `queue`. At most 64 sends wait; the
65th refuses its action. `refreshes` asks its resources again when the reply
lands; a mutation the source answers synchronously (without a request) has
landed, so its resources are asked in the sending commit. A send still in flight
does not ask them; what it writes shows before its reply only through the data
module's `overlay`, if it exports one (below). `then` is parameterless,
runs once at the host's next clock advance as a new commit (under the driver, an
input's own answer's `then` before the input's reply), reads the latest
answer, does not run for a failure that brought no answer, and cannot send its
own mutation; to repeat, use a task (see "Repeating while a condition holds").
Do not mistake the scheduling boundary
for a general async workflow or a per-reply event log.

A long poll (`/changes?since=…&wait=…`) is a stream too: add
`exactLongPoll: { cursor: 'since', from: 'seq' }` beside `exactStream`, and the
host asks again with each reply's cursor, skipping heartbeats and backing off
on failures (`docs/reference.md`, LLP 1016.000 D8). Re-read on each event with
`resource view = load() with live.seq`.

A bidirectional WebSocket uses a resource's TypeScript
`fetch(url, { exactStream(event, socket) { … } })` (LLP 1016.000 D7).
Keep the handle in module state by resource identity; after its `open` event,
a mutation source can call `socket.send(textOrBytes)` or `socket.close(code,
reason)`. The mapper may also send and close on its own socket while it
answers `open` or a `message` (a subscribe, an Ack per frame), with no
`fetch`. Sends run after the commit that settles the mutation or the frame
stands, in order, and have bounded admission. A mutation's source that
sends answers now or with a promise, and cannot also `fetch` (whichever
comes second refuses it). A resource's answer may send its own
request on its module's socket and await the reply, with no timer, and so
may a mutation whose source answers with a promise: what each turn of it
sends goes out once the commit that ran that turn stands, and stays sent if
it is let go (on the web build, only its source's synchronous call may
send). Refreshing or removing the
resource invalidates the old handle. **TypeScript on the web JS target,
macOS, iOS and Linux**; the wasm target and Rust sources remain
receive-only. See
[the reference](reference.md#generate-typescript-data-source-types) for bounds and events.

A large response needs a parse that yields. On native hosts each turn of an
answer has 100 ms: from its call, or from the `fetch` or storage step it
awaited, until it next awaits one. A turn over that is refused, so a parse
that passes its tests on a 300 KB thread fails on a 1 MB one.
`await scheduler.yield()` (the web's [Prioritized Task
Scheduling](https://wicg.github.io/scheduling-apis/#dom-scheduler-yield)) ends
the turn, and the answer goes on in a new one with its own 100 ms. Yield every
few thousand items in a parse or tree walk:

```ts
for (let i = 0; i < nodes.length; i++) {
  if (i % 2000 === 1999) await scheduler.yield();
  comments.push(toComment(nodes[i]));
}
```

The function that yields is `async`, and so is every caller that awaits it.
On the web the page has no such budget, and a yield lets it paint mid-parse.

### Optimistic writes: the overlay

To show a write before its reply, the mutation declares `refreshes` and the data
module exports an `overlay`:

```contract
shape Item
  text: string

component List
  resource items = loadItems() as shape Item[]
  mutation added as shape Item refreshes items
  action add(text: string)
    send added = addItem(text)
  view
    column
      for item in items key=item.text
        text item.text
```

```ts
import type { Overlay } from './app.contract.d.ts';
type Item = { text: string };
export const overlay: Overlay = (source, args, answer, writes) => {
  if (source !== 'loadItems') return undefined;    // other resources show their answers
  const shown = [...(answer as Item[])];
  for (const w of writes) {
    if (w.source !== 'addItem') continue;
    const [text] = w.args as [string];
    if (shown.some(i => i.text === text)) continue; // the answer has it
    if (w.answered) w.keep();                       // the server has not caught up yet
    shown.push({ text });
  }
  return shown as never; // `Result<S>` cannot be narrowed from a generic `S`
};
```

From the send, derives and views read `items` as the overlay's value. Without an
overlay they read the answer, and the write shows only once a refreshed answer
has it. The runner calls `overlay` for each resource a write affects, with:

- `source` and `args`: the resource's source name and arguments. Resources can
  share a source with different arguments; check `args` when a write belongs to
  one of them.
- `answer`: the resource's current answer. Copy it rather than changing it.
- `writes`: the sends that affect it, in send order. Each has `id`, `mutation`
  (the mutation's name), `source` and `args` (the send's), `reply` once its reply
  has landed, and `answered`: its reply has landed and `answer` was asked after
  that, so `answer` should include it.

A write shows until the first answer asked after its reply landed. If that
answer does not include it yet (a server that is not read-after-write
consistent), call `keep()` on it to go on showing it. The runner does not ask
again on its own, so a kept write shows until a later answer (from `refresh`,
another write's reply, or new arguments) that the overlay does not keep it over.
Returning `undefined`, throwing, or returning a value outside the resource's
shape shows the answer and stops showing every answered write; the journal says
why for the last two.

A write that ends without landing stops showing in that commit, with no undo
code, and the resource is asked again: its reply fails, a newer send to the same
mutation replaces it (a `queue` mutation's sends all stay), or an action assigns
the mutation's slot. If that ask fails, `failed(items)` is true and `items` keeps
its last answer.

- The overlay runs synchronously inside the commit, so it cannot await, and it
  has no effects: `fetch`, storage, the store (`get` included), `native` and
  `crypto` (`crypto.getRandomValues()`, `crypto.randomUUID()` and every
  `crypto.subtle` call) refuse with an error naming the call, the same on every
  host: a call that returns a promise rejects, any other throws. The
  runner reuses its value while its inputs are unchanged, so compute it from
  them alone.
- Writes are not saved: after a relaunch or a reload only answers show, and a
  resource a write was showing in is asked again.
- A source whose module runs on a worker shows answers only, said once in the
  journal.
- A Rust source implements `DataSource::overlay`, returning
  `Ok(Some(Overlaid { value, keep }))`, or `Ok(None)` to show the answer.
- The agent's `state` shows what the overlay shows; `state.writes` lists the
  writes still showing, as `{ id, mutation, landed }`, when there are any.

A failed resource retains its value or placeholder, with `pending=false` and
`failed=true`. Argument changes, refresh, or successful answers clear the failure.
A domain error returned as a record is an answer and must be handled as data.

Infer the actual source interface from the Contract:

```sh
bun exact.mjs contract types app.contract -o app.contract.d.ts
bun exact.mjs contract rust app.contract -o /tmp/shapes.rs
```

Use those generated declarations with the existing TypeScript/Rust integration.
A shape has no exported name in the `.d.ts`: name one by its source,
`type Recipe = Result<'recipe'>` (a list's element: `Result<'recipes'>[number]`;
an optional answer is `… | null`, so `NonNullable<Result<'find'>>`).
`app:/data`, `app:/cache` and `app:/tmp` exist on every host before a source
runs, so a file directly in one (`app:/data/notes.json`) needs no `mkdir`; a file
deeper down needs its folder first (`storage.fs.mkdir('app:/data/drafts')`, which
makes the folders above it too), or the write fails with `ENOENT`.
The [human guide's data-module section](contract-for-humans.md#writing-the-data-module)
has a complete `app.ts`: synchronous, `fetch` and SQLite sources, the grants
each needs (one per line: `['sqlite.open app:/data/books.db', 'net.fetch https://…'].join('\n')`;
`net.fetch` takes an `http` or `https` origin, `http://127.0.0.1:8080` too, or a
whole scheme, `net.fetch https:` (any host and port, for a server the person names
at run time; `net.websocket` takes `wss:` or `ws:` the same way); on iOS
cleartext `http` reaches only a local host, and only with `app.json`'s
`host.ios.localNetworking` set; a server on the local network needs the grant
`device.local-network purpose.<key>`, whose text is the platform's prompt on macOS
and iOS; an `iframe` of `http://` from a named host needs
`host.macos.appTransportSecurity` or `host.ios.appTransportSecurity` set to
`{ "allowsArbitraryLoadsInWebContent": true }`, which relaxes web views only and
not an `http:` sub-resource of the app's own `assets/` page),
and how to drive it with storage. The macOS driver launches the assembled app
bundle, so its ATS policy also applies during drives. A path grant covers its path and what is below
it, by whole names: `sqlite.open app:/data` covers `app:/data/inbox-amy.sqlite`,
`sqlite.open app:/data/inbox` does not. A refused open names the file it wanted
and the grant line that would admit it.
A token, a password or a key the module keeps is a secret, not a file: grant
`secret.keep <name>` (one line per name, `secret.keep signal.token`) and use
`store.set(name, value)`, `store.get(name)` (a string, or `null`) and
`store.forget(name)` in an answer (LLP 1018). It is the Keychain on Apple; on the
web, the page's `localStorage`, readable by any script on that origin; a Linux
launch keeps it only until the app exits for now. A drive keeps it, by default,
only in a named `--storage` store (`EXACT_STORE=real` gives an Apple drive, on a Mac, a simulator or a phone, the Keychain a normal launch reads).
A secret that only the person in front of the device may read (an account's token
behind Face ID) carries a policy on its line, `secret.keep token user-presence`
(`biometry-any`, `biometry-current-set`, `device-credential`), then `optional` when
the person chooses whether it is locked; the app grants `device.local-auth <purposeKey>`
and depends on `@exact/local-auth` (LLP 1133.010). Its value is never in the store:
`store.get` throws `InvalidAccessError` while it is locked, `await store.unlock(name,
{ reason })` shows the platform's sheet and answers the value, `store.has` and
`store.locked` say what is kept, and `store.protect(name)` and `store.unprotect(name,
{ reason })` turn an `optional` lock on and off. `store.set` keeps every write in the
name's state, a rotated token's too. Keep the value out of every answer: an answer is
kept and shown. `authenticate(reason=)` from the same package is the check alone: it
gates a screen, never a secret. The web keeps no locked secret yet: a name that must
be locked is refused there, and `localAuthStatus().presenceSecrets` is false. Under the
agent the sheet is a `presence` ticket (`tap @presence allow`, `cancel`, `fail`,
`lockout`, `invalidated`), after Face ID's `permission` ticket while it is `prompt`.
The compiler accepting a source call does not provide its implementation. Check
its arguments, declared result, grants, storage access, and bake-time behavior.
Keep generated output out of version control. Use app-local sources for domain
formatting or algorithms beyond the finite standard roster.

A bake runs initial data work and packages first-frame values. A first-frame
value is not the answer: every host asks the TypeScript module again at launch,
natively once it loads after first pixel, even a source with no arguments
(`logs`: `<resource> shows its build-time answer until its source answers`, then
`<resource> answered: …`). A resource with an argument (one after `with` too) made
from `exactTime()`, through a derive, another resource or a state an action
assigns it (or writes under a condition on the date), gets no first-frame value, since the bake's date is unknown: it shows
its placeholder until the module answers with the launch's date, so a task waiting
on `lib.loaded` never adopts an answer stamped at 1970. Don't pass the clock into a
source whose answer doesn't depend on it, or its first frame waits for the module:
format times in Contract (`formatTime(at, time.utcOffset, "short")`). A `send` an action makes before the module loads (a
document opened at launch) waits for it, `pending` meanwhile (`logs`: `send <name>:
waits until the data source is ready`). Live requests run after that under host scheduling. Do not assume a secret store, disk database,
or authenticated network session is available while baking. See
[the data-module reference](reference.md#generate-typescript-data-source-types).

## Views, layout, and interaction

For an attachment of any type, use `input type="file"` with no `accept` (or
`accept="*/*"`) and `multiple` when needed. `showPicker("attach")` opens the
input with `id="attach"`; `change` receives `Picked[]` with copies under
`app:/tmp/picked/`. A specific `accept` list remains bounded to image/video
types and the app's `file_handlers`. On Linux the agent can answer a held
picker, but the live picker is unavailable. See [the picker
spec](../llp/1069.002-media-picker.rfc.md#any-type-input-charlie-2026-10-11).

The view forms are element, component use, `if`/`else` (and `else if`),
keyed `for`, exhaustive option `match`, and `children`. Wrap root regions in a
stable element.
HTML names Contract spells otherwise (the compiler names each): `div` is `column`,
`row` or `view`; `span`, `p`, `strong`, `em`, `b` and `i` are `text`; `h1`–`h6` are
headings, as written; `label` is `text` beside its field, which
`aria-label` (or `aria-labelledby`) names; `img` is `image`; `a` is `link`; `ul`,
`ol` and `li` are a `list` or a `column` of rows; `title` and `meta` are `head
title=… description=…`; a `table` is `view display="grid"` with
`grid-template-columns` (or a `column` of `row`s); there is no `form` (a field's
Enter is its `submit`) and no `details` (keep `open` in state, show the body `if
open`). A view chooses with `if`, as an action does; a count is
`xs.length`, never `count`, `len` or `.size`; a resource's placeholder is `else
empty()` (`[]` for a list), never `else []`. A component
call uses parentheses; a built-in element uses space-separated attributes.
`button "Save" press=save` is text-child sugar; an explicit text child is useful
when that label needs its own styling or driver id. A `button` is the web's
`<button>`: a block whose content is centred in its height and whose text is
centred (`text-align: center`). Give a sized button no alignment rows; write
`display="flex"` (a row) for an icon and a label side by side, and
`text-align="start"` on a list row or card made of a button.

Use the CSS and HTML vocabulary. Defaults matter: a bare box is block and
content-box, while `row` and `column` supply flex styles. Do not emit React Native
property names, invented CSS aliases, or unknown HTML tags. Read the schema and
tag mapping for admitted properties and units. CSS standard behavior does not
imply that every browser API is implemented on every host.

For a control that should remain a Tab stop but be announced as unavailable,
write `aria-disabled=true`. It does not stop `press`; check the unavailable
state in the action. Descendant controls and inline links inherit this
accessibility state unless they explicitly say `aria-disabled=false`. HTML
`disabled=true` removes a form control from focus.
Projected Apple segments, UIKit bar items and titles, and AppKit menu items keep
that accessibility state on the native item people encounter. UIKit menu rows
and alert actions stay selectable but read as enabled; UIKit's public disabled
states would stop their input.

`class=Style` applies literal style values; node attributes win. Conditional
classes select two named styles with `class=(condition ? A : B)`. Computed values
go on nodes. There is no selector cascade, dynamic style object, or arbitrary
class-string composition. A style branch can mix a number and a CSS keyword in
a property's admitted value space; this does not add general union types.
On a number-only row such as `opacity`, a computed branch known to be a string
is refused at build time even when another branch is numeric.

`background-image` takes up to four linear, radial, conic or repeating linear
gradients; `mask-image` takes one. Stops take percentages, or px lengths for
linear/radial gradients (unitless zero is allowed). For example,
`background-image="repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)"`.
A repeating gradient’s px period stays fixed as a box resizes; mixed px/%
and omitted stops use CSS fix-up after layout. A zero period paints its last
stop, as Chrome does. `background-attachment="local"` is still
refused: separate row backgrounds restart at each row, and repetition alone
does not make a virtualized list’s background follow its content.
`contract vocab background-image` gives the grammar and limits. AppKit’s captured
hatch matches Chrome exactly; UIKit qualification is pending. On UIKit, repeating
backgrounds share the existing drawn-gradient limitation for
`background-attachment="fixed"` ([open issue](../issues/20261011-repeating-hatch-across-list-rows.md)).

For scrolling, provide a bound and inspect measured layout. For virtualized
lists, use `list virtualized=true` (no other tag takes it), one direct keyed
`for`, and one flow root per row. A vertical list needs `height`, `max-height`
or a growing `flex`, which may be computed (`max-height=(narrow ? "320px" : "640px")`).
A percentage bounds it only against a definite containing-block height (a plain
`column` is content-sized), and the bake refuses (`bake-scroll-unbounded`) only a
list with no bound at all, in its first 390×844 frame, so look at the list in each
layout it takes. It takes `estimated-item-height`. A horizontal one needs a
literal `display="flex"` and a literal positive `height`, takes
`estimated-item-width`, and refuses wrapping, reversed or right-to-left flow, a
nonzero `gap`, `justify-content` other than `flex-start`, and `reorderdrop`.
Main-axis padding (`padding-top`/`-bottom`, a row list's `-left`/`-right`) is
CSS's room before the first row and after the last, inside the scroll content:
room under a header laid over the list, or over a tab bar
(`padding-bottom="calc(env(safe-area-inset-bottom) + 49px)"`). It takes a
number, an `env()` length or its `calc()`, or a computed number, not a
percentage; the end that `reachend`, `scrollFollowEnd` and `scroll-start="end"`
reach is past it, and on iOS the pull-to-refresh spinner draws below
`padding-top` (a padding-bottom taller than the port has limits: LLP 1010
§6.9). `scroll-padding` (`-top`/`-bottom`, a row list's `-left`/`-right`, the
same forms) is where its `scrollIntoView` aligns a row, as CSS's snapport:
with `scroll-padding-top` equal to a header's height, the first row's
`block="start"` is `scrollTop` 0. Only a virtualized list takes it
(`lower-scroll-padding` elsewhere: native hosts read it nowhere else). `reorderdrop` belongs only on a vertical `list virtualized=true`
(each row's handle names it with `reorderFor`); the compiler refuses it on any
other element, where no host could drag. Lists that share a `reorderGroup`
(each with a `reorderdrop`, an `id` and string keys) exchange rows: the drop
fires once, on the list the row lands in, with the dragged row's key, then the
key it lands before (`none` at the end) and, for an action taking one more
parameter, `ReorderEvent { from, to }`. The action decides where the row goes: a board whose cards join the bottom
of the column they are dropped on ignores that key when `from != to`. A board
is columns in a plain horizontal `scroll`, each a header, a grouped list
(`flex=1 min-height=…`: the list is the drop target, where the lifted card's centre
is inside its scroll box, so space below a list that only fits its rows does not
take the card: a release there lands at the last gap the drag passed over, in the
card's own column when it crossed no other list) and its quick-add; give each grip `touch-action="none"`
and no `press`, `pan`, `pointerdown` or `key` of its own, so the host's keys
(Space, the arrows, Enter, Escape) work on it. The host draws the lifted row,
holds the drop until the move shows (a second at most) and scrolls the lists
near their edges; do not build card drags by hand. Lists nest one level deep; an inner vertical list needs a literal
`height` or `max-height`. Do not revive the removed legacy `item-height`
windowing mechanism. Rows inserted, removed or resized above what the reader
sees keep the reader's place, as CSS scroll anchoring does; a list at its start
stays there, so rows inserted on top show, and one following its end
(`scrollFollowEnd`) follows it. A row root may move where it paints
(`translate`, `rotate`, `scale`, a relative `top`/`left`, `z-index`: a lifted
row being dragged) and keeps its place in the list.

A `button` is the platform's own control by default. A background, border or
radius, rich children, or a row/attribute/context the native control cannot
support makes it your bare box instead. Classes count after merging, and a
row or incompatible child on any conditional arm keeps the whole button bare.
Grouped-list row buttons and their detail accessories stay bare by default so
UIKit can read the cell's face; explicit `appearance="auto"` carries a custom native control.
`appearance="none"` explicitly asks for your box. `appearance="auto"` explicitly
asks for the platform's button and refuses unsupported rows or children. The
switch must resolve to a literal after class merging; use `if` with two
buttons to switch. Its first `text` is the title, its second is the subtitle, and
its one `image "symbol:…"` is the symbol. Raster faces need
`appearance="none"`: the graphical plan is shared, and macOS and Linux do not
paint a native raster face yet. These are semantic face data, not layout
children. An image-only face needs a nonempty `aria-label`. With
`flex-direction="row"` (or absent), an image before/after the texts is
leading/trailing; with `flex-direction="column"` it is top/bottom.

Native buttons admit `font-size`, `font-weight`, `color`, `white-space`,
`line-clamp` and `text-align` on the button or its texts; a text's own row wins.
Apple uses only rows authored there, including classes; absent rows leave the
platform's font and colour alone. The web inherits the page's font and colour.
Tab/menu projections keep their existing ancestor `text-transform` on the
projected title, which the face query reads as painted.
`white-space="nowrap"` asks for one truncated line, `line-clamp=N` caps wrapping,
and `text-align="start"` places the face at the start of its box. The symbol
admits its own `-exact-tint-color`, `font-size` and `font-weight`; absent symbol rows
follow the title. Its `width`, `height` and `object-fit` are refused.

`gap` (or `column-gap` for a row, `row-gap` for a column) sets image-to-title
spacing. Absent means the platform's spacing, not zero; title-to-subtitle
spacing stays the platform's. `align-items` and `justify-content` accept only
`center`. `-exact-control-size` takes `mini`, `small`, `medium`, `large`;
`-exact-corner-style` takes `dynamic`, `small`, `medium`, `large`, `capsule`.
Both are style rows admitted only on native buttons, including in a class.
With explicit `appearance="auto"`, `border-radius` sets the authored radius
and wins over the named corner style; under the default it makes the button bare.
`padding` and its longhands set content insets. Absent leaves the native
control's insets/corners alone.

`pointer-events` takes `none` or `auto`. A disabled native button retains
its authored colours; bind `opacity` for authored dimming. It can open a
dialog or popover with `commandfor` and `command="show-modal"`,
`"show-popover"`, `"toggle-popover"`, or with `popovertarget`; targets may be
bound, and an empty target is no target. `href`, `action` and swipe attributes
remain refused. `-exact-enabled` transitions are not available.

**Exact2's menu rule** (LLP 1021): a row of a menu (`popover role="menu"`)
whose `popovertarget` (or `commandfor` with `command="show-popover"` or `"toggle-popover"`) names a
popover that is no menu opens it as if by the menu's own invoker (the button
that opened the menu, or the node its context menu opened on), as
`showPopover(id, source=)` does: not nested in the menu, so it stays open as the
menu closes on choose, beside where the menu came from. Write that popover
outside the menu: one written inside it is nested by where it is. A row naming
another menu opens its submenu, nested. The build notes each such row it can
see (literal ids, roles and actions: `lower-menu-row-popover`). Every other
row (a dialog's command, a custom one) keeps HTML's own activation. On the web
the rule takes the row as it is at the click: a press that changes a row from
naming a popover to naming something else still opens that popover, and one
changing it the other way leaves the popover nested.

On macOS, iOS and the web, an action can call `showModal(id)` or `close(id)`
for a `dialog`. macOS and the web follow the invoker's modal focus path; iOS
presents the dialog as an alert. Bare `close()` still closes the window.

On the Mac a `dialog` that is not a confirmation is a sheet on its window, as
iOS's is a sheet (LLP 1115.002 D7.2): AppKit centres it, dims the window under
it and keeps clicks from it. It is as large as its content, at most the
window's content area, and laid out as written (an authored `width` is used,
`width="100%"` is all of that area; its insets are not, since AppKit places
it). Escape and ⌘. close it, the one `buttonStyle="filled"` button answers
Return, `closedby="none"` leaves only its own buttons, and `closedby="any"`
also takes a click on the window behind it. A popover that is not a menu is
AppKit's popover beside its invoker (D7.3), with AppKit's arrow and material,
on the side its `position-area` names (`top …` above, `right …` beside, else
below): `popover="auto"` closes on Escape, a click elsewhere or another window,
`popover="manual"` only by its own buttons. Write no surface for either: the
sheet and the popover are the platform's. A popover of a dialog's is written
inside it, as in HTML (outside it, it is inert while the dialog is open). Under
the agent both paint in the page as before; `--chrome platform` on `macos`
shows them, `tap`, `type` and `tree` reach what is in them, and `screenshot
"file" window` draws them over the window.

`popover` alone also accepts an optional string. For example,
`popover=(expanded ? some("manual") : none)` keeps the same mounted box:
`some("manual")` adds the property, and `none` removes it. An empty string
still declares a popover; it does not restore inline layout. After setting
`expanded = true`, call `showPopover(id)` in the same action. Setting it back
to false closes the presentation and restores the box to its inline parent.
An iframe kept in that box, with the same `src`, retains its mounted guest.
Use explicit viewport bounds and admitted absolute positioning for an expanded
box; this does not add `position="fixed"`. The
[resident iframe fixture](../scripts/fixtures/iframe-resident-popover.contract)
shows the complete enter/exit action and layout.

For an invoker popover with an authored CSS area, use
`position-try-fallbacks="flip-block, flip-inline, flip-block flip-inline"`.
A fresh opening takes the first fitting attempt, retaining the base if none fits.
An open popover keeps its chosen option while it fits, including across resize;
if none fits after movement, it keeps the current one. Commas separate
attempts; spaces combine flips, applied in the order written. `flip-start`
mirrors across the top-left/bottom-right diagonal (`bottom span-right` becomes
`right span-bottom`), so `flip-start flip-block` (right, ending at the
invoker's bottom) and `flip-block flip-start` (left) differ. `none` disables
fallbacks. In a right-to-left document the diagonal runs from the top-right
corner. With no fitting option, CSS shifts the box back into the window. A
popover with an authored `position` is absolute in the top layer and is held
only at the page's start edges (the top, and the left, or the right in a
right-to-left document), so it can run past the others. The web and painted Apple layers implement these flips; an authored
fallback row selects painted placement on macOS for a non-menu popover, including
when added while open. Removing the row restores platform presentation; focus
and text selection survive either change. Native
menus and confirmations keep platform placement. Physical areas also accept
`top span-left`, `bottom span-left`, `left`/`right` with `span-top`,
`span-bottom` or `span-all`, and the four `top|bottom left|right` corners.
Logical areas, `flip-x`, `flip-y`, named rules and area-dependent sizing are
not implemented. Use fixed sizes, zero insets and default self-alignment
for native parity; percentage margins are not resolved by the native layer.
`flip-start` also exchanges `width` and `height` in CSS; the native layers keep
the laid-out size, so give a popover that uses it equal ones.

A route with `navigationPresentation="modal"` is the same sheet on the Mac
(LLP 1115.002 D7.4), as large as its content (an authored `width` or `height`
is used; its `inset` is not), under the window's content area. Its `header`
is the sheet's own: the heading a title row at the top, and at the bottom
right the `id="back"` control (Cancel) and then the one prominent button
(`buttonStyle="filled"` or `"bordered-prominent"`), which answers Return.
Anything else in the header (a search field, a menu's button) stays in it, at
the top of the sheet. Escape and ⌘. go back as the `id="back"` control does;
`closedby` is a dialog's. A heading inside a pressable keeps the route painted
in the window, and `logs` says so. `--chrome platform` shows the sheet: `tap
"add-save"` presses its button (`native: "NSButton"`), `back` dismisses it,
and `state navigation.sheet` names its title, its buttons and its size.

With no `buttonStyle` a button is the platform's default: plain tinted text on
iOS, a push button on macOS, bordered on the web. Say what the action is:
`buttonStyle="filled"` on the screen's main action, `"bordered"` on one that
must read as a button, nothing on a secondary or inline one; and
`destructive=true` on one that deletes, which iOS draws in the system red.
A round control that floats over content (a search or compose button in a corner, a
bookmark over a card) is `buttonStyle="glass"`, or `"prominent-glass"` filled with
the accent, with its symbol coloured by `-exact-tint-color`: iOS 26's own glass, not a
custom circle.
`buttonStyle` needs a native button; on a default button that comes out bare,
`lower-button-style` names the first reason and says to remove it or write
`appearance="none"` without `buttonStyle`. It is a styleable host-policy prop;
its names and allowable branches
are checked against `schema.json`'s `buttonStyles`. Backgrounds, borders,
shadows, filters, `font-family`, other typography or inner layout and
`-exact-press-scale` are refused with `lower-button-style-attr`, naming a custom
`button` as the alternative. A native button still takes size, place, opacity
and transform rows. Hosts implementing the measure hook supply its fitting
size before the first frame, including height-for-width; hosts without it
keep the existing intrinsic-size report. Test the actual
platform look; macOS can report stand-ins for gap, subtitle and wrapping.
See [`controls.rs`](../contract/lower/src/controls.rs) for the checks.

**Prefer native controls.** Write the Contract form and each host draws its own
control; a hand-built lookalike (a painted switch, a row of buttons for tabs, a
drawn title bar) is a bug. [`apps/shelf`](../apps/shelf/app.contract) is the recipe
to copy for an iPhone app: tabs, a large title with a bar button and header search, a
segmented filter, rows that push, a long-press menu, swipe to delete, the system
alert, a sheet with Cancel and Save, pull to refresh, grouped lists, persistence and
fetch, with its tests, in roles and text styles only. `bun scripts/no-tells.mjs <app>`
lists any literal colour, font size or weight an app's `.contract` files still write. On iOS:

| Write | iOS draws |
| --- | --- |
| `button` (`buttonStyle`, native by default) | `UIButton` |
| `list appearance="auto" listStyle="inset-grouped"` of `section`s (`header`, rows, `footer`) | `UICollectionView` list, as Settings ([human guide](contract-for-humans.md#choosing-a-native-button)); a row of your own parts starts and ends at the cell's margin, as UIKit's own rows do, so write no padding at its ends |
| the same, `listStyle="plain"`, each `section`'s `header` one letter (Contacts) | the list with UIKit's A–Z section index at its trailing edge, when every section has a row; a name that starts with no letter A–Z goes under `#`, not nowhere |
| `list` with no `appearance`, its rows down the page (each child is a row) | a plain list's cells: UIKit's separators, the cell's grey ground while a row is pressed, the row selected under the screen it pushed. Draw none of it, and no inset either: a row's content stands at the cell's margin (16 at each end; UIKit's, 20 in an iPhone's landscape) where you write no padding, the row `border-box` (LLP 1138); write a padding to choose your own. The row is your box; a border you write on a row is yours and replaces the separator there (`border-bottom="none"`: none), and `appearance="none"` keeps rows of your own (a transcript, a feed of cards). The Mac shows a table, the web and the Linux desktop one line between rows, Android none (LLP 1115.000 D16) |
| `delete=remove(id)` on either list's row (`disabled` beside it: no delete) | the cell's swipe action Delete, which a full swipe performs (`contract vocab swipe`) |
| `input type="checkbox" switch` | `UISwitch` |
| `input type="radio"` (the radios of one `name` are a group) | the SF Symbols circle, filled at its centre when checked, at body size; in a grouped list's row no circle but the row's checkmark, which a tap on the row moves (LLP 1115.000 D13.2) |
| `input type="range"` (as wide as what holds it; in a `row`, `flex=1 min-width=0` beside its label) | `UISlider` |
| `input type="date"`, `"time"`, `"datetime-local"` | `UIDatePicker`; with an empty `value` its pill shows your `placeholder` (`placeholder="Add a date"`) or, with none, no words, and a choice fills it (LLP 1115.000 D13.1; macOS's field likewise; the web shows the browser's own empty field). Android shows Material's filled field, the value as the phone writes dates and `placeholder` its floating label, and Material 3's picker, in the control's `accent-color` where it has one (LLP 1115.001.000) |
| `select` of `option`s | a pop-up button with its menu |
| `progress value=done max=goal` (a goal, an upload; as wide as what holds it) | `UIProgressView`, tinted by `accent-color` (LLP 1116 D8); never a drawn track and fill |
| `progress` (no `value`) | `UIActivityIndicatorView`, `.large` from a 37-point box (LLP 1069.001) |
| `popover="auto" role="menu"` of `button`s, opened by `popovertarget` (a row whose `popovertarget` names another menu: its submenu; one naming any other popover opens it from the menu's own invoker, exact2's menu rule) | `UIMenu`, nested (LLP 1021) |
| `dialog role="alertdialog" aria-label="Remove book?"` of a `text`, its action(s) and one Cancel; or a `text` and one closing button alone | `UIAlertController(.alert)`: the label its title, the text its message; the one-button alert when it has no action (LLP 1115 D6; recipe [below](#a-confirmation)) |
| any other `dialog` (a field, Save, Cancel), opened by `showModal(id)` or a button's `command="show-modal"` | a sheet as tall as its content, up to the large detent, its content laid out as written and centred across it (`width="100%" box-sizing="border-box"` fills it); `close(id)`, its `command="close"` button or a swipe down closes it. `closedby="none"` refuses the swipe, `"any"` also takes a tap on the page behind, and a dismissal the person made fires `cancel` (LLP 1115.000 D11.5). A form with a title bar is still a sheet route |
| `role="tablist"`: each tab a symbol over a label / one text or image | `UITabBar` / `UISegmentedControl`, the tablist at least its native height unless `min-height` says otherwise (LLP 1059) |
| `role="radiogroup"` of `button role="radio" aria-checked=…`, each one text or image (a value chosen from a few) | `UISegmentedControl`, as a tablist's; never a tab bar (LLP 1116.004 D5.2). Keys of your own design: a radio that paints itself (its own background, border or radius, LLP 1104 D2), or `appearance="none"` on the `radiogroup`, keeps its buttons as you paint them; on iOS and the Mac the group keeps one Tab stop (the checked radio) and arrow keys that press the next or previous radio (Left and Right swapped right to left); on tvOS the focus engine moves between them |
| a route whose first child is a `header` holding one heading (`text role="heading"`; an `aria-level` alone is not one), a text after it and its buttons | the navigation bar; a level-1 heading (`aria-level=1`) is a large title, in a sheet too, and any other level an inline one; the text after it the subtitle |
| a pushed route with no such `header` | a bar holding the system Back button alone (LLP 1115.000 D5.2) |
| a route with `navigationBar="none"` | no navigation bar: a full-bleed screen that draws its own way back; the edge swipe still goes back |
| a route with `navigationPresentation="modal"` | a sheet |

On macOS, date, time, datetime-local and select controls are default Tab stops,
independent of the system's Keyboard navigation setting. Their native control
owns focus and editing keys; `focus`/`blur` and cancellable `key` handlers still
address the Contract node. Disabled, inert and hidden controls stay out of the
loop; `tabindex=-1` allows explicit focus while excluding sequential focus.

`contract vocab <name>` lists each one's props. A route does not scroll by
itself: its content goes in a `scroll`, `list` or `overflow-y="auto"` box, which is
the bar's scroller with nothing written (`navigationScroll` names another; "Routes
and web documents"). A sheet's swipe down and a pushed screen's edge swipe press the
route's enabled control whose `id` is the root's `navigationBack`, or, when the route
has none, one outside every route (a header the screens share), which is optional: a
root with a `navigationKey` alone is the navigation root and names no control. The agent's
`back` presses it too, as Escape does on a Mac sheet. When a sheet's swipe down should not do what its
Cancel does (Cancel reverts the edits, a swipe down keeps them), give Cancel another `id`:
a swipe down then dismisses without pressing it, and on the Mac Cancel stays in the header,
out of the sheet's Cancel slot. A route with no such
control still has the platform's back button, edge swipe and swipe down (and the
browser's Back): they go back through the root's `navigate` handler with the location
beneath (`nav = go(nav, url)` pops to it), or, with no handler, the runner pops the
router itself, as `nav = back(nav)` would (LLP 1115 D5). On the web the browser's
Forward and a `link` to a declared route do the same with no handler: the runner goes
there itself, as `nav = go(nav, url)` would (LLP 1115.000 D4.3); a native host's link
still needs the handler. A declared but disabled
control refuses them, as does `closedby="none"` on a sheet. A route whose scroller opens
with a full-bleed picture says `navigationBar="transparent"`: iOS draws its bar clear at
the scroller's top and insets nothing, so the picture sits under the bar as under a fixed
transparent header on the web, and the bar's material comes as the content scrolls. On a pushed iOS
route under the platform bar, that control's text becomes the bar's back button title
beside the bar's own chevron (no text shows the chevron alone), so label it `Recipes`,
not `‹ Recipes`. With no such control the back button is UIKit's default, which since
iOS 26 is the chevron alone, the previous title unshown, as in a hand-built app; a title
there is the control's text, said.

Inside a route's `header`, wrap related bar buttons in a `row role="group"`.
iOS 26 keeps each explicit group's buttons on one shared glass background and
separates adjacent groups with UIKit's fixed-space item. Ungrouped buttons keep
the existing platform default. `buttonStyle="glass"` on a bar button gives it a
glass capsule of its own (`sharesBackground = false`), one for each button so styled. The wrappers remain ordinary layout on the web;
this adds no spacing value or custom button face.

A heading of only white space is no title on iOS, and a large title that starts empty
takes no height. To keep that height while the words load, write the heading as
`text "\u{200B}" role="heading" aria-level=1 color="transparent"`, the colour on the
heading itself (UIKit draws U+200B alone as quotation marks), and drop the colour when
the words arrive. Before iOS 26 a route pushed over it with no back control of its own
shows that title on its Back button.

The bar shows a header's heading, the first text after it (its subtitle), one search
field, one tablist and its buttons; anything else in the header is not shown on iOS. A
menu or `dialog` written in the header is its opener's, as on the web, not bar buttons.
On iOS the search field scrolls away with the content while it is empty, as UIKit's
does; `position="sticky" top=0` on the `input` keeps it in view (React Navigation's
`hideWhenScrolling={false}`). The route's scroller runs under the bars wherever its
menus, popovers and `dialog`s are written: they take no place in its flow.
The agent paints the authored header by default, but marks such a node in `tree` and
`screenshot`, and an `expect` or `tap` on it fails, saying so. A `testId` on a bar
button, the search field, a segment or a tab is its `accessibilityIdentifier`, which
XCUITest and Maestro address.

Say what a thing is and leave how it looks unsaid: no `background-color`, `height`,
`font-size` or `padding` on these, or the host draws your box instead of its
control (LLP 1115). Worked forms, each compiled:

```text
// The navigation bar: buttons before the heading are leading items, after it
// trailing; a popovertarget button is a menu item (⋯), a search input the bar's
// search field, a tablist the segmented title.
column navigationKey=`${e.id}` navigationScroll="inbox-list" position="absolute" inset=0 display="flex" flex-direction="column"
  header
    text "Inbox" role="heading" aria-level=1
    row role="tablist"
      button role="tab" aria-selected=(filter == "all") press=show("all")
        text "All"
      button role="tab" aria-selected=(filter == "unread") press=show("unread")
        text "Unread"
    input type="search" value=query input=search placeholder="Search"
    button popovertarget="inbox-menu" aria-label="More"
      image "symbol:more"
    button press=compose aria-label="Compose"
      image "symbol:compose"
  column id="inbox-menu" popover="auto" role="menu"
    button press=compose
      text "New Message"
  // Pull to refresh: a `refresh` handler on the scroller; `refreshing` holds the
  // spinner until the reload settles. A row's `delete=` is its swipe to delete.
  // A first load is a `progress` beside the list while it has no rows, never in a
  // row: a spinner inside a cell is something a hand-built iOS list never shows.
  list id="inbox-list" refresh=reload refreshing=pending(items) flex=1 min-height=0
    for m in items key=m.id
      button press=open(m.id) delete=remove(m.id)
        text m.title

// A subtitle: the first text after the heading (UIKit's own on iOS 26).
column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0 display="flex" flex-direction="column"
  header
    text "Shopping List" role="heading" aria-level=1
    text `${left} ${left == 1 ? "item" : "items"} left`
    button press=add aria-label="Add Item"
      image "symbol:add"
  scroll id="list" flex=1 min-height=0
    …

// A sheet's bar: Cancel is the root's navigationBack control, which a swipe down and the
// agent's `back` press; Done trails.
column navigationKey=`${e.id}` navigationPresentation="modal" navigationScroll="form" position="absolute" inset=0 display="flex" flex-direction="column"
  header
    button id="back" press=back
      text "Cancel"
    text "New Message" role="heading"
    button press=save
      text "Done"
  scroll id="form" flex=1 min-height=0
    …
```

`action reload` is `refresh items`; `pending(items)` is true while it runs.
A segmented control in the content chooses a value or switches panels: a value
from two to five (a preset, units, a period, a filter) is a `row role="radiogroup"` of
`button role="radio" aria-checked=…`, each pressing its value (the web gives it the
arrow keys and one Tab stop); switching panels, a `row role="tablist"` of text tabs
outside the header. A list row's `delete=action` is its swipe to delete, UIKit's on
iOS (`contract vocab swipe`). Other swipe actions are a hand-written swipe row, a
`scroll` laid out as a row (`display="flex"`) whose content iOS needs exactly the
scroll's size ([pitfalls](agent-pitfalls.md), "A hand-written swipe row shows its
buttons under the row"). Its swipe actions
are UIKit's, drawn by UIKit (rounded on iOS 26 and later) whatever the buttons
paint; when every control the row names says `appearance="none"`, the paint is
kept instead, in a grouped list too: the row swipes by its own scroll, as on
the web, with one row open at a time (a native swipe row's opening closes it,
and its drag closes a native one), a full swipe past half the row pressing the outermost action
(a haptic as it arms), and each action a VoiceOver custom action of the row's
content.

An `image` source is the same string on every host: a path under the app's
`assets/`, an `http(s)` URL, `symbol:<role>` (the roles are
[`schema.json`](../kernel/tables/schema.json)'s `symbols`; a player's are `play`,
`pause`, their `-fill`s, `skip-back-15`, `skip-forward-15`, `skip-back-30`,
`skip-forward-30`, `speaker`, `speaker-mute` and `moon`; an unknown role is refused
with the list), `symbol:sf/<name>` (an SF Symbol by its Apple name: drawn on Apple
only, blank on the web and Linux, where `layout` reports `reason: "platform"`; a
build of an app that ships either warns, its first use with the repair and each
later one in a line; a tab or button that must show everywhere takes a role or an
icon token), an `app:/data|cache|tmp/…` file
(a picked photo, or one the data module kept with `storage.fs`; it shows after a
relaunch too), or a `data:` URL of at most 1 MiB, past which every host shows
nothing (the web and Apple journal `image refused`). Shrink a picked photo for an upload limit with
`storage.fs.compressImage(path, to, {maxDimension, maxBytes})`, which writes an
upright JPEG with no location metadata (`type: "image/png"` writes a PNG that
keeps transparency; [reference](reference.md#shrink-a-picked-image-for-upload-storagefscompressimage));
Linux answers `unsupported`. Save an edit (a turn, a flip, a crop, a size) with
`storage.fs.transformImage(path, to, {rotate, crop})`, which keeps the photo's colour,
HDR and metadata where the host can and says what it wrote
([reference](reference.md#save-an-edit-of-a-photo-storagefstransformimage));
`compressImage` takes `rotate`, `flip` and `crop` too. Upload a file with `fetch(url, {method: "POST", headers:
{"content-type": "image/jpeg"}, exactBodyFrom: path})`, not `readFile` then `body`: the host
reads the file as it sends (under `fs.read`, at most 64 MiB), so a 2 MB photo never passes
through the answer's 100 ms step; a missing or denied file rejects the fetch, naming why. Keep a picked photo by copying it to
`app:/data` and answering that path; never tell hosts apart in the data module
(`HermesInternal`) to choose a source
([LLP 1069.002](../llp/1069.002-media-picker.rfc.md) D7, [LLP 1011](../llp/1011-image-v1.spec.md) §2).

An `image` hears HTML `<img>`'s two events, once per source, on the web, macOS and
iOS: `load=` when the picture is ready (no payload), and `error=` when it does not
load, with a `message` that says why (`error=failed` runs `action failed(message:
string)`; `error=failed("icon")` passes `"icon"` before it). On macOS and iOS the
message is the reason (`HTTP 404`, `Could not connect to the server.`, `not an image
format this host decodes`); the browser gives none, so the web says `the image did
not load`. A symbol fires neither. What each host's fetch of an `http(s)` source
sends, follows and accepts is LLP 1011's: macOS and iOS send no cookie and no
`Referer`, follow redirects to any host (within App Transport Security), take any
2xx body up to 64 MiB whatever its `Content-Type`, and keep an HTTP cache on disk;
the web is the browser's `<img>` (its cookies and `Referer` rules, no size cap). SVG
draws on the web only; on Apple it is an `error`. A tinted remote image on the web
needs CORS headers, or it is an `error` too. A drive sees both after `clock +<ms>
real`; `clock settle` can return just before them.

### Sound, MIDI and media

A sound effect is a declared WAV that an action plays ([LLP
1096](../llp/1096-sounds-an-app-can-schedule.rfc.md)): `sound "assets/…wav"` at the
top level (16-bit or float PCM, one or two channels, at most 10 s; the compiler
reads it), then `playSound(src, at=, gain=, group=, playbackRate=, pan=)` from any
action. A path built at run time (`` `assets/kits/${kit}/kick.wav` ``) plays only the
files a `sound` line declares, so declare each one it can name. Every call is a new
voice, so a retrigger is another call. `at=` is the runner's clock
(`performanceNow()`'s milliseconds; the past means now), `gain=` a linear 0–1, and a
`group=` is monophonic by start time: a voice ends where the next one in its
group starts, as a drum machine's choke does. `playbackRate=` (0.25–4, Web Audio's
name) moves pitch and length together, as tape does: at 2 a voice is an octave up and
lasts half as long. `pan=` places it from −1 (left) to 1 (right) by Web Audio's
equal-power law ([LLP 1125](../llp/1125-a-transport-midi-and-the-small-pieces.rfc.md)
D10). `stopSounds()` (or `stopSounds(group=…)`) ends what
sounds and cancels what waits. The web plays the first sound after the page's first
tap or key; Linux and Windows keep the record and play nothing.

```contract
sound "assets/ding.wav"

component Ding
  action ding
    playSound("assets/ding.wav")
  view
    button "Ding" press=ding
```

To keep time (a metronome, a drum machine, an interval timer, a rhythm game), declare
what should play with a `sequence` and let the runner play it ([LLP
1125](../llp/1125-a-transport-midi-and-the-small-pieces.rfc.md) D2–D5); do not
start hits from a timer ([a step sequencer](recipes/sequencer.md): lanes of their own
lengths, a kit, pause and resume, MIDI). It has a task's gate, and one `play(tracks,
bpm=…)`. A track loops at its own `beats`, `loops` times or forever (0), so tracks of
different lengths run against each other; a step is at a `beat` (a sixteenth is
`0.25`) and plays a sample, a haptic, or both. The runner issues the steps into
the voice table a second ahead on its own clock, so no action runs per step and a
busy frame does not move a beat. The arguments are read again after every commit:
change the pattern or the tempo in state and it plays from that commit, the tempo
without a jump in phase. `from=-4` with `countIn=` is a count-in; `ended=` runs an
action when a finite take ends, at that time:

```text
shape Step
  beat: number
  src: string
  gain: number
  group: string
  note: number
  velocity: number
  channel: number
  duration: number
  haptic: string

shape Track
  beats: number
  loops: number
  offset: number
  output: string
  steps: Step[]

fn click(beat: number, gain: number): Step = Step(beat=beat, src="assets/click.wav", gain=gain, group="click", note=-1, velocity=100, channel=0, duration=0.1, haptic="")

component Metronome
  state playing = false
  state bpm = 120
  resource transport = exactTransport() as shape Transport
  derive bar = [Track(beats=4, loops=0, offset=0, output="", steps=[click(0, 1), click(1, 0.5), click(2, 0.5), click(3, 0.5)])]
  sequence beat when playing
    play(bar, bpm=bpm, from=-4, countIn=[click(0, 1), click(1, 1), click(2, 1), click(3, 1)])
```

The shapes are flat and positional, as `playSounds`'s hit is: "none" is `""` or
`-1`. Keep the tracks in a `derive`, so a commit that changes nothing else
compares them at once. `exactTransport()` says what plays (`state`, `reason`,
`bpm`, `anchorAt`, `anchorBeat`); the beat now is `anchorBeat + (performanceNow() -
anchorAt) * bpm / 60000`. A playhead or beat light follows it as an animation, not a
frame task: a loop whose opacity is a computed `linear()` curve on a clock timeline,
re-read by a task keyed on the anchor ("Following the transport", below), so no app
code runs while it plays. A frame task is for text that changes
with the beat, gated on it being shown, whose action writes the beat, not the time, so a
frame that moves no beat commits nothing. The gate turning
false stops it; `stopSounds()` silences what sounds and the next steps still play.
For one or two scheduled hits, `playSounds(hits)` with `at=` is enough (a list of a
shape whose fields are, in order, `src`, `at`, `gain` and `group`, then optionally
`playbackRate` and then `pan`).

**Following the transport.** A playhead or a beat light that follows `exactTransport()`
is an animation, not a frame task: each light loops over its cycle (a bar, a lane) and
Core Animation or the browser plays it, so no app code runs while the music plays.
`level` passes a `linear()` curve's output through as the opacity, so a computed curve
says when in the cycle the light is lit; a negative delay puts the cycle where the
transport is; a clock timeline keeps a light mounted later in the same phase.

```contract
timeline Transport
keyframes level
  from opacity=0
  to opacity=1

shape Transport
  state: string
  reason: string
  bpm: number
  anchorAt: number
  anchorBeat: number

shape Step
  beat: number
  src: string
  gain: number
  group: string
  note: number
  velocity: number
  channel: number
  duration: number
  haptic: string

shape Track
  beats: number
  loops: number
  offset: number
  output: string
  steps: Step[]

fn pct(t: number, cycle: number): string = `${round(t / cycle * 10000) / 100}%`
// Lit through the first `span` ms of each `cycle`, a cycle starting `start` ms along the
// beat grid; `pos` is where the transport was when the loop mounted.
fn lit(span: number, cycle: number, start: number, pos: number): string = `level ${cycle}ms linear(1 0% ${pct(span, cycle)}, 0 ${pct(span, cycle)} 100%) ${0 - (((pos - start) % cycle) + cycle) % cycle}ms infinite`

component Beats
  state playing = false
  state pos = 0
  state synced = ""
  resource transport = exactTransport() as shape Transport
  derive anchor = `${transport.anchorAt} ${transport.anchorBeat} ${transport.bpm}`
  // `transport.bpm` is 0 before anything has played; a derive may not be Infinity.
  derive beat = 60000 / max(1, transport.bpm)
  sequence click when playing
    play([Track(beats=1, loops=0, offset=0, output="", steps=[])], bpm=120)
  // Where the transport is, read once per play, seek or tempo change.
  task follow when transport.state == "playing" key=anchor
    after(1, sync)
  action sync
    pos = transport.anchorBeat * beat + performanceNow() - transport.anchorAt
    synced = anchor
  action toggle
    playing = not playing
  view
    column padding=16 gap=8
      button press=toggle testId="toggle"
        text (playing ? "Stop" : "Play")
      row gap=8 role="img" aria-label="Beat"
        for i in range(4) key=i
          view flex=1 padding=4 border-radius=4 position="relative" background-color="-exact-tertiary-fill"
            if transport.state == "playing" and synced == anchor
              for g in [synced] key=g
                view position="absolute" inset=0 border-radius=4 background-color="AccentColor" opacity=0
                  animation=lit(beat, 4 * beat, i * beat, pos) animation-timeline=Transport
```

- **What stays a frame task:** text that changes with the beat ("Count-in 3"), gated on the
  transport states that show it. Its action writes the beat, `floor(anchorBeat +
  (performanceNow() - anchorAt) * bpm / 60000)`, never the time: the runner compares state
  after an action, so a frame on which the beat has not changed commits nothing and draws
  nothing (`apps/metronome/app.contract`'s `readout`). Read the position in an action (a
  pause's) as `anchorBeat + (performanceNow() - anchorAt) * bpm / 60000` beats.
- **A light that is not just on or off** puts every stop in the curve. Over a four-step cycle,
  a trail ember lit the step after the head and fading from 0.4 to 0.18 through it is
  `linear(0 0% 25%, 0.4 25%, 0.18 50%, 0 50% 100%)`; a fade after the beat is a few stops
  sampled from its easing.
- **A one-off change at a beat** (dim two beats after a touch) is a finite animation whose delay
  is that beat's distance from now; a second, identical `keyframes` name restarts it, as CSS
  restarts an animation only when its name changes.

Tests read the runner's record: `expect sound has "assets/ding.wav" at 0`, with any
of `gain`, `ends`, `playbackRate`, `pan` and `by end|group|cut|stop|cancelled|edit`, or
`expect sound missing …`; a sequence's steps are voices like any other, so `clock
+2000` then `expect sound has "assets/click.wav" at 1500` works with no audio, and
`expect state transport.state == "playing"` reads the transport.
`state sounds` lists the last 64 voices, `state transport` the transport.

MIDI is Web MIDI's, and the runner keeps its queue as it keeps the voices (LLP
1125 D6–D8). Grant `device.midi purpose.midi`, a line of the data module's grants
(`export const grants` in `app.ts`, `grants()` in Rust) whose second word is a key of
`strings/en.json`: `"purpose.midi": "…"`, the text the person is shown when asked. Read the ports with `resource midi = exactMIDI() as
shape MIDI` (`state`: `prompt`, `granted`, `denied`, `unavailable`; `outputs` and
`inputs`, each `{id, name, manufacturer, state}`; `sent`, the last 128 messages that
went out in time order, each `{output, data, timeStamp}`, Timing Clock left out: an activity log; a track whose `output` is `""`, and `sendMIDI("", …)`, go to no device but are listed with `output: ""`, so the log shows what plays with none connected), ask for them from a press with
`requestMIDIAccess()`, and let the person pick an output (a `select` of
`midi.outputs` by `id`). Then a step's `note` plays on its track's `output` (a
note-on, and a note-off `duration` beats later), `clockOut=output` on `play`
sends clock, Start and Stop, and `sendMIDI(output, [144, 60, 100], at=…)` sends
whole messages from an action. Hear a controller with `midimessage=heard
midiinputs=[id]` (the action may take a `MIDIMessageEvent`: `input`, `data`,
`timeStamp`, `beat`). To keep a device's time instead, `clock=input` on `play`
(a `select` of `midi.inputs`): it waits (`transport.state == "waiting"`) until the
device sends Start, then plays at the device's tempo (`transport.bpm`), and its
Stop leaves it waiting again. Nothing to schedule by hand: a host hands each message to the
device 50 ms before it is due, so an edit or a stop takes back what has not gone.
Test it with no device: `prefer midi.outputs "OP-XY"` then `tap "connect"`, and
`expect midi sent "OP-XY" 99 4c 7f at 0` (bytes in hex) or `expect midi missing …`;
`tap midi "keys" 90 3c 64` is a message from an input, `tap midi "keys" clock 125
1000` a second of its clock at 125 bpm; `state midi` lists the
queue ([Metronome](../apps/metronome/app.contract) is whole).

A video is HTML's `video`, not a native view ([LLP 1042](../llp/1042-video.spec.md)):
`video "assets/intro.mp4" poster="assets/intro.jpg" autoplay=true muted=true loop=true
playsinline=true object-fit="cover"` is a looping hero, which Apple draws with an
`AVPlayerLayer`. Give it a `poster` (a still of its first frame): AVPlayer takes 50 ms
to a second to draw its first frame after launch, and the poster shows until it has.
`audio` stays HTML's player for long media
(a song, a podcast: `video`'s props and events with no picture, LLP 1042 §8).

A media source is the string an `image` takes, `app:/data` files included: an
episode the data module downloaded with `storage.fs` plays on every host but
headless Linux, which has no player. An empty `src` fails (`error`
`src-not-supported`), as HTML's does, so render the element once it has a
source. A bound `currentTime` seeks when its value changes; to seek to the same
time again (a skip back, "start over", a scrubber let go where it was grabbed)
call `fastSeek(id, seconds)`, which seeks each time it runs. `load(id)` loads the
source again: a retry after an `error`, or a file written since. Both name the
element by its `id`, as `focus` does.

To be the system's Now Playing app (the media keys, the lock screen, Control
Center, a headset's buttons, the browser's media hub), give the player the Media
Session's `metadata=MediaMetadata(…)` and bind its actions as the element's events
([LLP 1098](../llp/1098-the-media-session.rfc.md)):

```contract
component Episode
  state paused = true
  state seek = 0
  state at = 0
  state part = 1
  action ticked(t: number)
    at = t
  action skipBy(sign: number, d: MediaSessionActionDetails)
    seek = max(0, at + sign * d.seekOffset)
  action seekAt(d: MediaSessionActionDetails)
    seek = d.seekTime
  action restart
    seek = 0
  action nextPart
    part = part + 1
  action hostPaused
    paused = true
  action hostPlaying
    paused = false
  view
    audio "assets/episode.mp3" paused=paused currentTime=seek timeupdate=ticked
      metadata=MediaMetadata(title=`Episode 12, part ${part}`, artist="The Show", album="", artwork="assets/art.png")
      seekbackwardOffset=15 seekforwardOffset=30
      seekbackward=skipBy(-1) seekforward=skipBy(1) seekto=seekAt
      previoustrack=restart nexttrack=nextPart
      pause=hostPaused playing=hostPlaying
```

- Every field is named (`album=""` when there is none); `artwork` is one image,
  an `https:` URL or an app asset path.
- The six actions are `setActionHandler`'s names: `seekbackward`, `seekforward`,
  `seekto`, `previoustrack`, `nexttrack`, `stop`. Each offers a last
  `MediaSessionActionDetails` (`action`, `seekOffset`, `seekTime`, `fastSeek`) to
  take or leave. A bound action is an offered control; the lock screen shows no
  skip button for an app that does not bind one.
- `seekbackwardOffset`/`seekforwardOffset` are what the lock screen shows and
  the `seekOffset` when the platform gives none (default 10). Keep an action that
  ignores the record (`seekforward=skip(30)`) at the offset it declares.
- Play and pause are the element's own: the platform plays and pauses the player
  and the app hears `play`, `playing` and `pause`. With `paused` bound, mirror
  them, as above.
- A Mac's previous and next keys send `previoustrack` and `nexttrack`, not the
  seeks: bind both.
- One element owns the session: the claimant that most recently started playing,
  kept while it is paused.
- On iOS the build refuses a claimant without `"audio_session": "playback"` and
  `"audio"` in `host.ios.backgroundModes` in `app.json`.

Tests trigger an action as the platform would and read what is published:
`tap "audio" mediasession "seekto" 600` (the seconds are a seek's `seekOffset`,
`seekto`'s time), `expect mediasession title == "Episode 12, part 1"`, `expect
mediasession has "nexttrack"`; `state mediaSession` is the whole record.

### Markdown editing and colour

A Markdown editor is a `textarea` with `markup="markdown"`; a `text` with it is
the reader (one node; the value stays the source string). Its toolbar is
`retainFocus` buttons calling `format(id, command[, argument])`, and its
`select` event hands an action the formats at the selection, for active states
([Markdown](contract-grammar.md#markdown-markup-format-select): the commands and
the `MarkdownSelection` fields):

```contract
component Editor
  state body = "# Title"
  state bold = false
  action edit(v: string)
    body = v
  action selected(s: MarkdownSelection)
    bold = ` ${s.formats} `.includes(" bold ")
  action embolden
    format("editor", "bold")
  view
    column
      button (bold ? "Bold (on)" : "Bold") press=embolden retainFocus=true
      textarea id="editor" value=body input=edit select=selected markup="markdown"
      text body markup="markdown"
```

Mentions, tags and citations drawn as pills in that editor and reader are
inline atoms: declare `atom` kinds by their source form, list them in `atoms=`,
give each source a face with `atomFaces=` and hear `atompress=`
([inline atoms](contract-grammar.md#inline-atoms-atom-atoms-atomfaces-atompress)).
Keep them out of your own rules over `setSelectionRange` and `beforeinput`: the
editor keeps a caret out of an atom and deletes it whole.

Choose a colour by what it means, not by how it looks. **The accent is for
what the user can act on**: a link, a plain button's title or symbol, a filled
button's fill, a toggle's on state, the icon of a tappable row. Write it as
`AccentColor`, never as the colour the accent happens to be today
(`-exact-system-blue`): the platform keeps the accent dynamic and dims it to
grey behind an alert or a sheet to say the screen under it is inert. **A
colour of its own is for what means something by itself**: status
(`-exact-system-green`, `-exact-system-red`), text hierarchy (`-exact-label`,
`-exact-secondary-label`), data, and marks. A colour of its own never dims; on
iOS the tint does where UIKit draws it, whoever set it (the root's
`accent-color` or a node's own), as UIKit greys a view's `tintColor`:
controls, bars, symbols drawn in the tint, and a box shadow in `AccentColor`.
A gradient or SVG paint in `AccentColor` is resolved by Exact to the app's
accent and does not dim. Set the app's accent once, on the root:
`accent-color` on the root element is the app's accent, and on iOS it is the
window's tint, as an AccentColor asset is, so every `AccentColor`, bar, sheet
and alert follows it. On Apple hosts only, an app may instead ship its accent
as an `AccentColor` colour set in its asset catalog
(`modules/apple/*.xcassets`, below): the build names it in the app's
Info.plist (`NSAccentColorName`, as Xcode does), and it is the accent wherever
the root's `accent-color` is `auto` or absent (UIKit applies it as every
view's tint; macOS's AppKit uses it where the user's system accent is
Multicolour). The web, Linux and Windows never read it: there the root's
`accent-color` is the accent the controls take, and the web's `AccentColor`
keyword stays the browser's, so an app that wants one accent everywhere writes
it on the root.

Keep `id` and `testId` separate:

- `id`: host command target, geometry, cross-node references.
- `testId`: driver/test target and stable inspection name. On the web it is
  `data-testid`, not the DOM `id`: a brief that asks for element ids wants `id=`
  (and `testId=` too where tests target the node).
- `for` key: data identity for row lifetime, not either node id.

Events bind actions (`press=save`, `input=edit(item.id)`). Captured arguments come
before host payload arguments. Do not add an event object or a JavaScript closure.
`navigate` is special: no captured arguments and zero or one location parameter.
`transformgeometry` and `transformrelease` must be paired. The complete payload
matrix is in [events](contract-grammar.md#events).

Known host commands are statements, not expression-returning functions. Several
commands have specialized argument checks; some generic commands leave validation
to the host. Do not infer that compiler acceptance proves a command's arguments
work. Use the command's working fixture and inspect its host result.

`relaunch()` restarts the standalone macOS process after its orderly quit,
including the storage drain; it works with the development menu off. A
`beforeunload` cancellation cancels the restart. The web reloads the page.
iOS/tvOS, Linux and native agent sessions journal a refusal; for a scripted
restart, start another drive on the same `--storage`. See [host commands](contract-grammar.md#host-commands).

### Bar buttons, glass and the search field

- iOS 26 puts adjacent ungrouped bar buttons on one glass capsule. `buttonStyle="glass"`
  on each gives it a separate capsule of its own; a `row role="group"` keeps some on one.
- A `popovertarget` button is the item's pull-down menu. Write the menu beside the header;
  one written in it is still its button's menu, never more bar buttons.
- The header's `input type="search"` hides when scrolling while it is empty, as UIKit's
  does; `position="sticky" top=0` on it keeps the search field visible.
- A header's buttons that press, and its disabled ones (dimmed, as UIKit's), are bar
  items; a readout nothing presses ("● Clock") is a `row` or `text` with `role="status"`
  ("Tabs and stacks"). Past the subtitle, the first text after the heading, other texts
  and boxes are hidden on iOS (`lower-header-unplaced`).

### A horizontal strip in a sheet

`scrollIntoView(id)` scrolls every scroller above the element, as the web's does, so
scrolling a strip of choices to one moves the sheet's form too. `block="nearest"` leaves a
scroller that already shows it alone and `inline` places it across the strip:
``scrollIntoView(`icon-${i}`, block="nearest", inline="start", behavior="instant")``.

### Native views: accessibility

A native view ([recipe](recipes/native-views.md)) that says nothing to VoiceOver is one
element under its box's `aria-label`, with the traits of its `role`: `button`, `link`,
`img`, `switch` or `checkbox` (`aria-checked`), or `slider` (`aria-valuenow`). A view with
parts sets `accessibilityElements` (`UIAccessibilityElement`s with a label, `.button` and
`accessibilityActivate`); the box is then a group under the label.

### Native views: a package's

A package serves a tag by declaring it in its `exact.json` `views` (LLP 1133.001 D1): an
app that depends on the package places it with no `app.json` entry. Its implementation
makes it in each session, the same instance that answers its names: Swift
`view(_:id:props:events:)` on the `ExactPackage` class, a `roster` of view classes from the
web implementation, Kotlin `ExactExtension.view`; `id` is the node's. A view declared
`"displayed": true` is told `displayed(id, shown, origin)` when it shows or hides, and a
prop marked `"binds": true` `bound(id, prop, origin)` when it changes; `origin` is `app`,
`tool` or `wake`, that of the commit that did it. `packages/fixtures/swatch` is a whole one.

## Arrange existing content on iPhone Duo

On iOS 27.1, an element hatch can call `element.arrange(using: controller)` with a
`UIArrangementViewController` configured with `UISplitArrangement`. Exact places the
marked element's two authored children in the controller and lays out each in its
allocated bounds. Give the marked element a definite size, such as `height="100%"`
in a sized route; allocated children do not give an auto-height parent its height.
The content, controls, styles, and bindings stay the same, including when the child
order or identities change. Keep navigation outside the arrangement and use a
bounded scroller in each child for independent scrolling. The first visible
vertical scroller is the default title and scroll-to-top target; `navigationScroll`
can name another. Pass `nil` to restore the authored layout.

A `true` result accepts the request; UIKit lays it out when the owning container is
available. `false` means the element is unavailable, it does not have two valid
children, or the supplied controller already belongs to another container.
An accepted request uses CSS layout while the child count is not two and resumes
when two valid children return.

To keep pane content still as the shared navigation bar minimizes, set
`route.controller.navigationItem.navigationBarMinimization.safeAreaAdjustment = .disabled`
in the route hatch (iOS 27+). UIKit retains the safe area while the bar minimizes
([Apple API](https://developer.apple.com/documentation/uikit/uibarminimizationsafeareaadjustment/disabled)).

Declare the hatch for iOS in `app.json`. On older systems, keep the authored CSS
layout; a single set of components can stack in a compact layout and sit beside one
another when expanded. See Apple's [arrangement guidance](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo).

## Routes and web documents

Declare at most one `routes <slot>` in the app's root file. Each row names an
absolute path pattern, optional tab membership, parentage by indentation, and
optional rendering/activation policies. `notfound` is a bare fallback: with none, the
compiler writes it and a "Page not found" screen with a way home (the location keeps its URL);
write it to draw your own. Do not redeclare the implicit router state with `state`.

Assign the results of `open`, `push`, `replace`, `go`, `select`, or `back` to that
router state. Use `path("route", args…)` to construct checked locations. A
template literal as a location is refused (`route-template`) and a string literal
is checked against the table, but any other computed string is not checked, so
always build locations with `path()`. A route's parameter fields are strings, with
empty strings for absent fields. `select` takes a tab name, not a URL.

`for entry in stack(nav) key=entry.id` gives retained screens their identities.
Bind the host's navigation root and per-entry `navigationKey`s as the
[router fixture](../contract/corpus/routes.contract) demonstrates. Test back,
tab switching, the same-URL push, external navigation, and route parameters.

Exact's web servers use declared routes for file-like deep links: `/profile/:handle`
admits a navigation to `/profile/photos.zip`. Existing public files win; unmatched
file paths stay 404, even with `notfound`. External static hosts need their own rewrites.

For a custom scheme, declare `protocol_handlers` in `app.json`, for example
`[{"protocol":"web+notes","url":"/open?u=%s"}]`. The Apple hosts register
that scheme. An incoming `web+notes://auth/callback#done` reaches `/open` with
the complete original URL percent-encoded in `u`; read it with
`searchParam(top(nav), "u")`. The handler needs an `/open` route and a
navigation root with `navigate=`. At cold launch it is the initial location;
when already running, each link fires one `navigate` event in arrival order.
An app without `protocol_handlers` keeps ordinary URL-to-location behavior.
A drive on macOS or iOS delivers a link as the system would, without iOS's
"Open in …?" alert: `type <root> "web+notes://auth/callback#done"`.
For a packaged-only scheme, use
`[{"protocol":"notes","url":"/open?u=%s","distribution_only":true}]`.
Apple archives and `--distribution` builds register it; local builds omit both
its scheme and template. `--bundle` or `--release` alone does not enable it.
Web builds copy this key into the web app manifest, without `http` and
`https`; the browser decides which schemes it supports and whether the
installed app is registered.
A Mac web browser declares `https` and `http` together, each with the same
`url` and the same `distribution_only`: a web link, and an HTML file Finder
opens in the app (as its `file:` URL), reach that template. The person makes
the app the default browser in System Settings. iOS registers neither, and
`host.<platform>.urlSchemes` refuses both
([reference](reference.md#run-an-app-from-a-terminal-on-macos), its table).

Every route (a node with a `navigationKey` under the root) is a direct child of the
root or of a `role="tabpanel"` in it — through `for` and `if`, never another
element: the compiler refuses one behind a wrapper (`lower-route-place`). Make each
route `position="absolute" inset=0`: the hosts hide a covered route, as
`visibility: hidden` does, so an in-flow route still takes its room. A covered
route stays mounted (its nodes are in `tree`, marked `inactive`), so a `testId`
two screens share names two nodes: give each screen its own (`list-error`,
`detail-error`).

A route does not scroll by itself, as a `div` does not: content taller than it is
never seen. A fixed shell (a map, a camera, a chat whose composer stays put) is
fine; anything else puts its content in a `scroll` or `list` with `flex=1
min-height=0` right after its `header`:

```text
column navigationKey=`${e.id}` position="absolute" inset=0 display="flex" flex-direction="column"
  header
    text "Inbox"
  scroll flex=1 min-height=0
    …
```

That scroller is the route's content scroller with nothing written: the first in the
route that scrolls vertically. On iOS it goes under the bar and collapses a large
title, which happens when it is the route's child right after its `header` (in a route
with no header, its first child); with content between them (a switcher, a filter, an
error line) the title keeps still and that content sits below the bar, large title
included; a tap on the status bar, or on the selected
tab at its root, scrolls it to the top. A route with several scrollers names a
different one with `navigationScroll`, by its `id`. The compiler refuses a literal
name that no element of the route carries, or one whose element is a box that never
scrolls on y (`lower-route-scroll`).

## Tabs and stacks

Tabs with a stack each have one layout that works on web, macOS and iOS
([the tabs fixture](../contract/corpus/tabs.contract), driven by
`host/web-js/conformance/tabs.steps`):

```text
main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow display="flex" flex-direction="column" height="100%"
  column flex=1 min-height=0 position="relative"
    for t in nav.tabs key=t.name
      column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
        for e in t.stack key=e.id
          column navigationKey=`${e.id}` navigationScroll=`content-${e.id}` position="absolute" inset=0 display="flex" flex-direction="column"
            header
              if e.name == "item"
                button id="back" press=back
                  text "Home"
              text e.name role="heading" aria-level=(e.name == "item" ? 2 : 1)
            scroll id=`content-${e.id}` flex=1 min-height=0
              …
  row role="tablist" display=(top(nav).name == "full" ? "none" : "flex")
    button role="tab" aria-controls="panel-home" aria-selected=(nav.tab == "home") press=pick("home")
      image "symbol:home"
      text "Home"
  if toast != ""
    button position="absolute" … // a root overlay: after the tablist, over everything
```

- A tab is built the first time it is selected, as UIKit and SwiftUI load a tab: until
  then its panel is empty (`unopened` in `tree`), and the components in it have no
  state yet. Once built it stays, with its stack, drafts and scroll. Resources and tasks
  are the root component's and run whichever tab shows. A drive or a test taps a tab
  before it reads or presses anything inside it; a target in an unopened tab is
  refused with the tap that opens it (LLP 1075.003 §9.14).
- Each route starts with a `header` (its heading, and on a pushed route the
  `id="back"` control), which iOS makes the navigation bar. Leave the tablist's and the
  routes' backgrounds, heights and colours unsaid: the platform draws the bars and the
  page. For an authored design, put `appearance="none"` on the `header` or root
  `tablist` to keep that bar's layout, colours and type on iOS too. A header
  button with `appearance="none"` stays authored in place while the rest
  of the header still projects. A title button keeps its contents; an authored Back
  replaces UIKit’s Back. Search or a header tablist can opt out on that element
  (a tab button alone does not opt its parent out of segments). Tabs retain their
  panels and stacks; other styling alone does not opt a header out. On iOS 26 and
  later neighbouring bar items share one glass capsule, as UIKit groups them;
  `buttonStyle="glass"` on a header button gives that item a capsule of its own
  (`sharesBackground = false`).
- A header button is its bar item: a symbol is its image and a text its title, in
  the bar's type, the platform's. A `font-size` or `font-weight` on a native button or
  its text sets that title's font, as on a button in the page, and its glass fits the
  words (`buttonStyle="glass" font-size=12` is a small "● Clock" chip). A face that
  says more keeps what it says: a status dot (a childless filled box) beside its
  label, or a label in a `font-family` of yours (which makes the button a custom
  one), is drawn as written, in its colours, as the
  item's image inside the bar's capsule (a "● Clock" chip in 12-point monospace).
  A filled box holding a text or symbol (an avatar, a badge) is drawn without the
  capsule. Leave the font unsaid for an ordinary Edit or Done.
- A readout nothing presses (a "● Clock" or "Saved" beside the title) is
  `role="status"` in the header, beside the buttons. (A `button disabled=true` is a
  dimmed bar item, UIKit's disabled one, which VoiceOver reads as a button that cannot
  be pressed, not as a status.) A readout is an item on its side of the heading, a custom view in the bar's
  glass drawn as written (its fonts, colours, a dot's size and corners, the gap) that
  follows the state in place, read by VoiceOver as static text (its `aria-label`, else
  its words). Its texts are never the subtitle. The web, the Mac and the agent's own
  chrome draw it where it stands in the header (LLP 1075.003 §9.15):

  ```
  header display="flex" align-items="center"
    button id="back" press=back
      text "Settings"
    text "Activity Log" role="heading" aria-level=2
    row role="status" aria-label=(live ? "Clock, receiving" : "Clock") display="flex" align-items="center" gap=6
      column width=7 height=7 border-radius=3.5 background-color=(live ? "c.cyan" : "c.dim")
      text "Clock" font-family="Menlo" font-size=12 color=(live ? "c.cyan" : "c.dim")
  ```
- Each tab names its panel with `aria-controls`; the panels are the stacks, and every
  tab's stack stays mounted, so a pushed screen, a draft and a scroll offset survive
  a visit to another tab. A tab is `select(nav, name)`; selecting the shown tab
  again pops it to its root. A sheet is a route of the tab's stack, rendered in the
  panel's `for e in t.stack` like the others, and is presented over the tabs; with
  tabs the root's own children are no routes, and one with a `navigationKey` is refused.
- On iOS the panels become a `UITabBarController`: a tab of one symbol over its label
  is its bar item, a filled box holding a text is the item's badge, and the
  tablist's `accent-color` (inherited, as in CSS) tints the selected item; `tap tab-…`
  works on every host. An app's `bun exact.mjs agent ios` and `test ios` drive what a
  person sees (`--chrome platform`; `--chrome agent` paints the authored tablist and
  header instead, as `scripts/agent.mjs` does by default): UIKit's tab bar, navigation bars and sheets show,
  `screenshot "file" window` draws them (`display` reads the simulator's screen as it
  is, a system alert over the app included), and a tap naming a tab or a header control
  presses it through its bar (`native: "tab-bar-item"`, `"bar-button-item"`). The
  bars' own motion runs on real time (`clock +300 real` before a screenshot); the
  software keyboard, another window, is not drawn.
- On the Mac a root tablist placed as above (the root column's last child) is the
  window's sidebar: AppKit's source list, one row a tab, with its button in the
  toolbar and View ▸ Show Sidebar. Under the agent on `macos` the authored tablist
  paints (on iOS only when `scripts/agent.mjs` runs alone: `exact.mjs` shows UIKit's
  bars); `--chrome platform` on `macos` shows the sidebar, and a tap naming a tab
  presses its row (`native: "sidebar-row"`).
- On the Mac a route's `header` is the window's title bar and toolbar: its heading the
  window's title (under a `head title`, over the app's name; there is no large title),
  its buttons toolbar items, a button whose `popovertarget` is a menu a menu item, a
  tablist of text tabs the centred segments, its `input type="search"` the toolbar's
  search field, and the `id="back"` control the bar's own Back. A header holding
  anything else (a picture, a second text, a pressable title, a button drawn as a box)
  stays in the page as written, whole, and `logs` says what it was. Under the agent
  the authored header paints; `--chrome platform` on `macos` shows the bar, `tap` and
  `type` naming a header's button, tab or search input go through it
  (`native: "NSToolbarItem"`), and `state window.toolbar.header` names its title, its
  items by test id and which are in the overflow menu. Its search field arrives with
  AppKit's own animation (`clock +1200 real` before a screenshot).
- Hide the tab bar on a route with `display="none"` on the tablist. Never remove the
  tablist with `if`: without it the root has no tabs and the panels' routes are
  found by no host.
- Root children after the panels box (a toast, a timer strip, a full-screen menu) paint
  over the routes and the native bars on every host, as later siblings do in CSS.
- A modal route (`navigationPresentation="modal"`) is opaque: its unsaid
  `background-color` is `-exact-background` (the grouped one when it scrolls a grouped
  list), so the route under it, dimmed, never shows through; write one to say another.
  On iOS that unsaid ground is the sheet's: below its large detent the sheet is
  UIKit's glass, as a hand-built one is, and at the large detent the system
  background. A `background-color="transparent"` is clear at every detent.
  Its corners are UIKit's: on iOS 26 and later a large sheet is rounded like the
  display and inset from the bottom edge, as Apple's own are. A `border-radius` on the
  route is the sheet's corner radius (its top-left one; `preferredCornerRadius`), which
  also seats a large sheet flat on the bottom edge.
  One pushed over a sheet (a confirmation over a dialog)
  is a sheet over that sheet, on iOS presented from it; Back returns to the sheet
  under it, and popping both closes both. Under the agent's frozen timing (the
  default) on iOS, a `tap` or `type` that opens or closes a sheet, its answer's `then`
  included, waits up to two seconds for UIKit to finish it; a reply with `settled:
  false, reason: "transition"` says it had not. `--timing platform` leaves that to
  `clock settle`.
- A sheet's heights are `navigationDetent`, space-separated words: `large`,
  `medium`, a point height, a percentage of the sheet's maximum height (`"45%"`,
  React Native's `sheetAllowedDetents: [0.45]`; above 0% and at most 100%) or
  `fit-content` (the route's content height; a menu or
  a short dialog), which goes alone or as `"fit-content large"`. Write a share of
  the screen as a percentage, never points computed from a measure. Heights go
  shortest to tallest on every screen, as UIKit takes them: before `large`, which
  goes last, use one kind (points, or percentages with `medium` as half), each
  taller than the one before; `"300 45%"` crosses on a small screen. `"100%"` is
  `large`, and a point height a screen's maximum reaches, with a taller detent after
  it, is left out there. A percentage, like a point height, leaves out the bottom
  safe area, which UIKit adds below. A literal with
  another word or order is refused; a bound one iOS cannot use is journaled (`logs`)
  with the route, and the sheet keeps its other detents or opens `large`.
  `fit-content` measures the route laid out alone, its
  height left to its children, so nothing sized from the sheet counts; on iOS every
  viewport unit (`vw`, `vh`, `vmin`, `vmax`, and kin) is the window's in every sheet, never the
  sheet's, so `min-height: 100vh` opens the sheet at its maximum; a route
  that scrolls is measured by its scroll extent, so give its rows
  `flex-shrink: 0`. The route does
  not pad `env(safe-area-inset-bottom)`: UIKit adds that band below the detent.
  iOS alone sizes a sheet (LLP 1075.003 §9.11).
- A sheet opens at its first height, the shortest, as UIKit opens one, unless the
  route says `navigationInitialDetent`, one of its `navigationDetent` words:
  `navigationDetent="medium large" navigationInitialDetent="large"` opens tall with a
  half-height stop (React Navigation's `sheetInitialDetentIndex`). It is read when
  the detents are set, so changing it alone does not move an open sheet.
- A sheet's grabber is the platform's unless the route says `navigationGrabber`:
  iOS draws one on a sheet with several detents, which it resizes, and none on a
  sheet of one height; `"visible"` or `"hidden"` says otherwise (UIKit's
  `prefersGrabberVisible`). Draw no grabber of your own.
- Without tabs, the routes are the root's own children, laid out the same way.
- Tests reach a tab by `tap`, or deliver a location as `type <root> "/saved"` (LLP
  1038 D11), which calls the root's `navigate`. `back`, an operation and a test
  step, is the platform's Back on every host (LLP 1116.006 D4): an open dialog's
  cancel, else the top sheet dismissed, else the pushed route popped, as iOS's Back
  button, the browser's Back or ⌘[ goes; with nothing to go back from it fails. On
  the web `tap <root> history -1` is the browser's history alone (native hosts
  refuse it).

A `head` node supplies document metadata. The innermost active value wins for
each field. `head edited=dirty` marks a document with unsaved changes (the dot in
a Mac window's close button; nothing elsewhere). Before a window closes or the app
quits, every element with a `beforeunload` handler hears it, DOM's event: an action
that calls `preventDefault()` keeps the window open — the browser asks "Leave
site?", a Mac app asks its own question — and `close()` closes the window once
it is answered (studio diary R17). Any other element's `title` is HTML's tooltip. `scroll document` declares page scrolling. Route `render`/`activate`
policy belongs to the site's renderer/build pipeline; follow
[the document fixture](../contract/corpus/document.contract) and
[LLP 1048.003](../llp/1048.003-documents-in-contract.spec.md). Do not claim SEO,
streaming, or deployment correctness from a client-only screenshot.

## Time, motion, graphics, and platform facts

A root task has one `every(ms, action)`, `after(ms, action)`, or
`every(frame, action)` entry, on the line under it:

```text
  task ticker mount
    every(1000, tick)
```
 The action is parameterless. Millisecond intervals
are whole-number literals of at least 1. The frame form has no delta-time argument and does not
catch up missed display frames. For deterministic tests, use the driver's clock.
The clock moves in advances, and one advance commits at most 4,096 timer actions
and `then`s (`TIMER_FIRE_LIMIT`), those that change nothing included; the rest is
refused, what committed is kept, and the clock stays at the last one's time. A
fast `every` under a long `clock +N` can reach it: tick slower or move the clock in steps.

A task with `when` (a gated task), such as
`task hide when toast != "" key=toastUntil` with `after(5000, expire)`, has its
timer only while the gate holds, as an `if` arm has its nodes, and a new key
restarts it, as a new `for` key makes a new row
([LLP 1092](../llp/1092-sends-that-queue-and-timers-that-wait.rfc.md)). Nothing
runs when the gate changes: turning true arms the timer from that commit's time,
turning false drops it, and an idle task keeps no host awake and commits
nothing at rest. `key=expr` alone means `when true key=expr`. An `after` fires
at its deadline, its action seeing `performanceNow()` equal to it under a test's or the agent's
clock and no earlier on a device: clear without re-testing the time (a strict
`performanceNow() > until` can do nothing there). A repeat that missed ticks while the app was
suspended fires once on a device, so measure time with `performanceNow()`, never by ticks. The
gate is a bool and the key a string, number or bool; neither may read `performanceNow()`
(`analyze-task-gate-clock`): gate on state and let the timer measure time. A
toast, a debounce (`when draft != saved key=draft` with `after(800, save)`), a
round's tick (`when screen == "play"`), a flight's frames
(`when flying` with `every(frame, step)`) and a sheet's opening
(`when top(nav).name == "icons" key=top(nav).id` with `after(1, reveal)`) are each
one gated task, and each is the root's: a child component declares none
(`type-child-resource`), so a sheet a component draws has its task in the root.

**Repeating while a condition holds.** Use a task with `when` and `every`. The
timer runs only while the condition is true. Any action that makes it false
stops the timer.

```text
  state pulsing = false
  state dim = false
  task pulse when pulsing
    every(1200, step)
  action step
    if dim
      dim = false
    else
      if busy
        dim = true
      else
        pulsing = false
```

The first `step` runs one interval after the condition turns true. Do not build
a loop from mutations: a `then` cannot send its own mutation
(`analyze-then-self-send`). For a purely visual loop, use a CSS `animation`
instead.

`performanceNow()` is the runner's clock in milliseconds since this launch of the app (it
restarts at 0 on every launch, and a window opened later reads the same clock; the driver's clock under the agent, from 0), as the web's
`performance.now()`, not a date: a deadline of `performanceNow() + ms` sent to a server is
in 1970, and one stored is meaningless after a relaunch. Anything stored or sent takes
`time.epochAtZero + performanceNow()` ([timer recipe](recipes/timer-that-survives-relaunch.md)). For the date, read the reserved
`exactTime` source and add `time.epochAtZero + performanceNow()`. There is no
`now()`: it is refused (`type-now-renamed`) with those two repairs. Its fields, which a shape declares as it reads them:
`epochAtZero` (Unix milliseconds when `performanceNow()` read zero), `utcOffset` (minutes east
of UTC), `locale` (BCP 47, the first preferred language), `formatLocale` (BCP 47, the
regional format the platform's own formatters use: macOS and iOS's Region, so English
with a Korean region is `en-US` and `en-KR`; `locale` where a host keeps none apart),
`timeZone` (IANA), `resolvedLocale` (the language of
the string table the app shows, `""` with no tables) and `seed` (a whole number
below 2^32, drawn once per launch, so `%` and `>>> 0` take all of it; mix a round
into it in 32-bit steps, as the [random recipe](recipes/random-and-shuffle.md) does). A read does not itself schedule a future render, and a
derive that reads `performanceNow()` is not read again as time passes, and when it is read
again differs by host. For a displayed value that must follow the clock, keep the
time in state that a timer's action (`task … every`) writes. Prefer `clock settle` to waiting for a transition in real time.
`time.utcOffset` is the zone's offset *now*: every host answers it again when the
offset at the clock's instant changes (a DST change, a new zone), checked before a
timer fires, so a midnight timer after the clocks change reads the new offset.
Under the agent it is the drive's zone at the virtual date, checked after each
`clock` (the JS target also before each timer inside one). It is not the offset
of an arbitrary timestamp: to show a past or future instant across a DST change
in the viewer's zone, format it in TypeScript with
`new Intl.DateTimeFormat(time.locale, { timeZone: time.timeZone })`.

Use admitted CSS transitions and keyframes. Check which properties animate and
which require optional capabilities. `-exact-spring(…)` (a `transition` timing
function), `-exact-exit-animation`, `-exact-layout-transition`, and presentation timelines have
specific documented behavior;
they do not admit arbitrary frame callbacks or a second app-state graph.

For drawing and pointer-tracking, `pointerdown`, `pointermove` and `pointerup`
hand an action that takes it a `PointerEvent` (`offsetX`/`offsetY` from the
node's content box, `buttons`, `pressure`, `pointerType`, `pointerId`, and
`clientX`/`clientY` from the viewport, `frame()`'s space), on any
node, a canvas included; set `touch-action="none"` on a drawing surface. Any
button goes down (`buttons` 2 is a right-click's), and a `contextmenu` action may
take the same record, where the click was. `wheel` hands a `WheelEvent` (deltas,
modifiers; a trackpad pinch is a Control-held wheel) and `preventDefault()` keeps
the scroll from happening; `drop` hands a `DragEvent` whose `files` are `doc:`
handles of any type, a folder's included, and takes a file drag over a field
inside its node; `dragenter` and `dragleave` bracket the drag for a drop
overlay; a `paste`'s `ClipboardEvent` carries `files` too. See
[Pointer](contract-grammar.md#pointer) and [Clipboard](contract-grammar.md#clipboard).

Use `keycapture=shortcut` on a container when its shortcut must run before
the focused field's `key=` handler. Capture runs outermost first, then the
target's capture and `key` handlers, then `key` bubbles outward. A capture
handler can call `stopPropagation()` to stop later handlers, including the
target's `key` handler when called there. `preventDefault()` keeps the field
from editing. The container gains no Tab stop from `keycapture` alone.

`key` is the DOM's keydown and `keyup` its keyup; an action that takes it hears
a `KeyboardEvent` (`metaKey`, `code`, `repeat`, `isComposing`, …). An Enter or
Escape handler at a field skips `e.isComposing`, as web code does: on the web
the Enter that picks an input method's candidate arrives composing. Track a held modifier as a
page does: set it from the event on every keydown and keyup, and forget it when
the window loses the focus, since a key let go in another app sends no keyup
(the page's `window` `blur`). `exactPage().hasFocus` turning false gates the task
that forgets it, and the hint shows only while the window has the focus:

```contract
shape Page
  hasFocus: bool

component Hints
  resource page = exactPage() as shape Page
  state meta = false
  action held(k: string, e: KeyboardEvent)
    meta = e.metaKey
  action forget()
    meta = false
  task release when meta and not page.hasFocus
    after(1, forget)
  view
    column key=held keyup=held testId="list"
      if meta and page.hasFocus
        text "⌘ held"
```

Drive it with `type "list" key "Meta" down`, `prefer has-focus false` and
`clock +1`. While an input method composes text in a field on macOS or iOS, a
modifier's own keys still reach these handlers, and a ⌘ chord commits the
composed text first (its `input`, then the chord's `key`), so a composer's
⌘Enter `key` action sends what was typed; a button that declares ⌘Enter is not
pressed then. See [Keys](contract-grammar.md#keys).

A long-press or right-click menu is a `popover` the node names with
`contextPopover="<id>"`: its `button` rows (with `popovertarget="<id>"
popovertargetaction="hide"`) and `hr` separators are the menu, and one row with
`contextPreview=true` is the preview, whose `press` is what tapping the preview
does (open the conversation). iOS presents UIKit's context menu, with the row
lifting and the preview popping into the screen its press pushes; macOS an
`NSMenu` without the preview; the web and the agent open the popover anchored to
the node. The node's own `contextmenu` action runs first, so one popover can
serve every row of a list. A node inside an open popover opens its menu over
that popover, which stays open, as HTML nests them. The agent opens it with `tap <node> contextmenu`
([LLP 1021](../llp/1021-menus.rfc.md) §5.1).

#### A confirmation

"Remove book?" with a message, Cancel and a destructive Remove (React Native's
`Alert.alert`) is a `dialog role="alertdialog"`: its `aria-label` is the
title, a `text` the message, each action a `button` with `press` that closes
it, and one Cancel, a closing `button` without `press`. Open it from a button
(`commandfor="<id>" command="show-modal"`) or from any action with
`showModal("<id>")` — a context-menu row's `press` asks before it deletes:

```
action askRemove(id: string)
  pending = id
  showModal("remove")
…
dialog id="remove" role="alertdialog" aria-label="Remove book?"
  text (title + " will be removed from your library.")
  button press=remove(pending) destructive=true commandfor="remove" command="close"
    text "Remove"
  button commandfor="remove" command="close"
    text "Cancel"
```

iOS shows the centred alert, its Cancel kept; the web its modal `dialog`;
macOS `NSAlert`, a sheet on the window, its Cancel on Escape and Return on the
one `buttonStyle="filled"` button that is not destructive, when there is exactly
one (LLP 1115.000 D7.6). Text, a cancel and at most three buttons make an
alert; several actions with no text (an "Open in…" chooser) make an action
sheet, unanchored on an iPhone (its Cancel drawn) when it has a Cancel and a popover
at its invoker when it has none, so a tap outside can dismiss it (LLP 1115 D6).
An alert has no outside, on every host (LLP 1115.000 D13.4): Escape and Back
cancel it (the Mac's one-button alert excepted, below) and a tap or click beside it does nothing, which is `closedby="closerequest"`,
implied for the alert's shape, so leave `closedby` unwritten. Write
`closedby="any"` only to ask for the platform's dismissible form in its place:
an action sheet on an iPhone (a popover on an iPad), a dialog a click outside
closes on the web, a sheet a click on the window behind it closes on the Mac, a
Material dialog a tap outside cancels on Android.
`close("<id>")` from an action closes it as its Cancel does.

An alert that only tells ("Couldn't save the note." and OK) is a `text` and its
one closing button, with no action: iOS's alert with that one button, macOS's
`NSAlert` with it as the default (Return presses it and Escape does nothing, as
on AppKit's own one-button alert), the web's modal `dialog`, Material's dialog
on Android. Under `closedby="any"` it is the dialog's own sheet on the Mac, which a
click outside dismisses; iOS keeps the alert, since an action sheet drops a
lone Cancel in an iPad's popover. The action that learns of the failure sets the state its `text`
reads and calls `showModal("<id>")` ([error-alert](recipes/error-alert.md)).

A prompt (a rename's, `Alert.prompt`) is the alert with a text field: an
`input` among its rows, which counts as its message when it has no `text`:

```
dialog id="rename" role="alertdialog" aria-label="Rename Pattern"
  input value=draft input=editDraft placeholder="Name"
  button commandfor="rename" command="close"
    text "Cancel"
  button press=rename disabled=(draft == "") commandfor="rename" command="close"
    text "Save"
```

iOS shows `UIAlertController` with the field (`addTextField`), macOS `NSAlert`
with it as the accessory view, the web the `dialog` with the field on a line of
its own. The field is the authored `input`'s, going through its machinery: the
input's value writes reach it (what is typed stands until the page writes the
value), its `placeholder`, keyboard, `disabled`, `readonly`, `maxlength` and
`beforeinput` are its, each keystroke is its `input` and the end of editing
its `change`, so the action's `press` reads what was typed. Return is one
rule on every host: the input's `key` handlers hear it first (and may prevent
it); a field with a `submit` handler then commits and runs it, and that is
all, as a hand-built field whose delegate handles Return; any other commits
its edit and presses the first action that is not destructive,
whatever `buttonStyle` says; while that action is disabled Return does nothing, so a Save with `disabled=(draft == "")`
opens dimmed and enables as a name is typed. iOS draws no action bold, as a
hand-built alert does not, unless the action Return presses says
`buttonStyle="filled"`: then it is the alert's preferred action, bold. (An alert
with no field prefers its one filled action that is not destructive, as the Mac
makes it the default button.) Only the alert's shape takes
one field at a time: with it, a cancel and at most three buttons, no `for`
(`lower-alertdialog` says which row breaks it).

A menu row's second `text` is its item's grey subtitle on iOS when it is a line
under the first (two texts in a block `button`, or a `flex-direction="column"`
one), with or without an `if` around it; two texts side by side in a row, or one
positioned `absolute` (a count at the corner), stay one title.

A submenu is a row whose `popovertarget` names another menu popover (`Copy ▸
path / link`): a submenu `NSMenuItem` on macOS, a nested `UIMenu` on iOS, and
on the web and under the agent the nested popover, opened by `tap <row>`. Place
it beside its row with `position-area="right span-bottom"`; give its items
`popovertarget="<outer menu id>" popovertargetaction="hide"` so a choice closes
the whole menu; write no `press` on the row that opens it (a native menu never
runs it); and draw the web's `›` as an `aria-hidden` text, since the native
menus draw their own arrow ([LLP 1021](../llp/1021-menus.rfc.md) §5.2).

For the Mac menu bar, put `-exact-menu="app"` on an ordinary button to add
an item without a shortcut. Literal destinations are `about` (directly under
About, where Check for Updates… goes), `app`, `file`, `edit`, `view`, `go`,
`window`, `help`, and `none` (no item: the chord stays a shortcut);
explicit placement overrides the usual `aria-keyshortcuts` chord filing. Its
accessible label or text titles it, and `disabled` disables it. An item in the
app menu or Help is the app's, so it stays enabled while no window is key and
under a sheet or a Contract modal dialog. Keep the button reachable: the agent taps it,
and other hosts show that ordinary button, with no application menu.
On the root, `-exact-menu-remove="show-tab-bar autofill close-all"` removes
selected standard items. Its whitespace-separated list must be literal; unknown
ids and non-root use are refused. The fixed ids are `close-all`, `autofill`, `show-tab-bar`, `show-all-tabs`, `enter-full-screen`,
`minimize`, `zoom`, `bring-all-to-front`, `show-previous-tab`, `show-next-tab`,
`move-tab-to-new-window`, `merge-all-windows`, `back` (Go ▸ Back), `develop` (the
Develop menu; its chords stay), `paste-and-match-style`, `find`,
`spelling-and-grammar`, `substitutions`, `transformations`, `app-help` (Help ▸
*App* Help), `actual-size`, `zoom-in`, `zoom-out`.
Other standard items remain; other hosts ignore removal. View has Actual Size
⌘0, Zoom In ⌘+ and Zoom Out ⌘-, which scale the root font size, the app's
`setRootFontSize` included, and leave `px` as it is. See the grammar’s “The
Mac’s menu bar”.


`frame(id)` and `measure("literal-id")` are action-only geometry reads returning
`Geometry` (`x`, `y`, `width`, `height`, `provisional`, `unavailable`). Handle `unavailable` and `provisional`. `frame` reads the last layout's border box
where the viewer sees it, as `getBoundingClientRect` does: in the viewport, through
every `translate`, `rotate` and `scale` on it and above it (the bounding box of a
turned box), with every scroll offset above it (the page's too) applied, so a drop
target needs no scroll bookkeeping and a dragged card is where it shows. Natively a
transform in flight counts at its end value; the web reads it mid-flight. `measure`
reads the border box as if its `height` were `auto`, at the same origin, with every
other style kept: its in-flow content, plus its padding and border, within its
`min-height` and `max-height`, and the height opposing `top` and `bottom` insets
give an absolutely positioned box. So a navigation root whose children are all
absolutely positioned, with no padding, border or `min-height` of its own,
measures 0 however tall it is laid out. Read a laid-out
size, the screen's included, with `frame`, and size a sheet as a share of the
screen with `navigationDetent="45%"`. Before the first layout both answer
`unavailable` with zeros, as on the web until the page's geometry code loads after
first paint; natively a mount task's `after(1, …)` runs after that layout.
Neither is a computed style binding to run every render.
`elementFromPoint(x, y)` names the front-most of the same boxes at a viewport
point by its nearest `id` (`option<string>`), through the same transforms. For a
point read that sees through an overlay, `elementsFromPoint(x, y)` lists every
hit node's own id, front to back, including hit ancestors; id-less nodes are
skipped (except an id-less hit outside its named ancestor's box includes that
ancestor once). `querySelectorAll("[data-paint~=\"accent\"]")` lists the own ids of
mounted, displayed, non-inert nodes bearing the declared `data-paint` word in
tree order. Covered routes and unselected tabs do not answer; a split list
answers with its detail where the host presents it (the web at desktop widths).
Closed popovers and dialogs do not answer; open ones do. Point reads skip inert
subtrees and use the presenter's placement for open top-layer content. Both reads
run only in actions and give one snapshot. Use `frame(id)`
for each returned id when drawing outlines.
For a drag still built by hand, test the dragged node's visual centre (the middle of
its `frame(id)`, which includes the `translate` the drag gave it), not the
pointer: the box under a lifted card is the card.

SVG uses SVG names. Native SVG geometry has incomplete support for `calc()`,
`min()`, `max()` and `clamp()`, including nested SVG viewports and `<use>`
placement and sizing. Use numeric lengths or percentages there; CSS layout math
on the outer SVG element does not imply support inside its scene.
SVG attribute-only lengths accept user units (unitless), `px`, `in`, `cm`, `mm`,
`pt`, `pc` and `%`: line/linearGradient `x1`/`y1`/`x2`/`y2`, radialGradient
`fx`/`fy`/`fr`, marker `markerWidth`/`markerHeight`/`refX`/`refY`, and each item
of SVG text/tspan `x`/`y`/`dx`/`dy`. Relative units and function text such as
`calc()`, `env()` and `var()` are refused in literals and literal template parts
(LLP 1055.000 D4). Literal concatenations are checked together; an incomplete
fragment that could finish an accepted unit is left to runtime. Literal choices in
conditionals, matches and inlined derives are checked too. Bound values must use
the same units. Marker `refX` also accepts `left`/`center`/`right`, and `refY`
accepts `top`/`center`/`bottom`; native hosts currently place these keywords at 0.
`foreignObject` renders directly on the web, but draws nothing through SVG `use`.
The compiler warns at a `use` whose unambiguous, unconditional literal target contains
one (`lower-svg-use-foreign-object`). Native hosts refuse `foreignObject` at run
time; for content on every host, position an ordinary box over the `svg`. Canvas 2D calls
live in a data module, and GPU/game surfaces in their optional module. A
hyphenated native tag must be listed in `app.json`'s `modules` (the bake refuses
others) and implemented by the module; its unknown attributes pass through
unchecked as the module's props. A known attribute binds to the module's box, which
takes layout, box and paint rows, handlers, `testId`, `id`, `role`, ARIA, `disabled`
and `inert`; any other known name (`color`, `value`, `command`, `href`) is refused,
so give the module prop another name. Do not
turn a missing widget or canvas operation into invented Contract syntax.

Apple native modules can include asset catalogs beside their Swift sources:
`modules/apple/*.xcassets`, or `ios/modules/*.xcassets` / `macos/modules/*.xcassets`
when the app has a platform-local module directory. The local directory replaces
`modules/apple` for that platform; tvOS uses the iOS directory. The Apple build
compiles these catalogs together with its generated assets into the main bundle.
On macOS, a catalog makes the dev build and agent launch the assembled `.app`.
Load custom symbols with `UIImage(named: "donut", in: .main, with: configuration)`
or `NSImage(named: "donut")`; SwiftUI uses `Image("donut", bundle: .main)`.
`symbol:sf/...` still looks up system symbols only. A colour set named
`AccentColor` is the app's accent on Apple hosts (`NSAccentColorName`; the
accent section above). Catalog changes require a
new native build; they are not web assets or live update payloads.

An Icon Composer project is selected explicitly by `host.ios.icon` or
`host.macos.icon` in `app.json`, as an app-relative `.icon` directory; tvOS
does not inherit it. It compiles with the catalogs and replaces the native
raster icon from `icons`, which still supplies the web. See the
[icon inputs in the reference](reference.md) for conflicts and fallback.

An iOS native module that uses `canOpenURL` to check for another app lists
its schemes in `app.json`: `"host": { "ios": { "queriesSchemes":
["comgooglemaps", "waze"] } }`. The build writes `LSApplicationQueriesSchemes`.
List at most 50 schemes, without `:` or `://`; changing the list requires a
native rebuild.

A pressed look is the host's, as UIKit's highlight is (LLP 1061 D7):
`-exact-press-opacity` (0 to 1, a share of the node's own opacity),
`-exact-press-background-color` and `-exact-press-color` (any colour row's
values: a token, a role, `light-dark()`), beside `-exact-press-scale`, with
which they compose. The host shows them while a finger or the mouse holds the
node (the innermost pressable, inside its box; sliding off, a cancel or a pan
lets go), eased in over 120 ms, at once under the agent's clock, so `"tap row
down" "screenshot x.png" "tap up"` shows it. Nothing waits on the runner and no
state changes. `-exact-press-color` reaches the node's text as `color` does
(a text with its own colour keeps it). Any of them, 1 included, replaces the
platform's unsaid press: iOS's dim, the Mac's darkening, a list row's cell
highlight. An `:active` state part (below) says the same paint, with an
opacity that is what the node shows pressed rather than a share; write each
paint one way or the other, not both. These are paint only; a press that
changes layout is app state. Do not draw a press from
`pointerdown`/`pointerup` state: each touch then waits on the runner. Say
nothing to get the platform's own.

Haptics are already there (LLP 1077 D14). `-exact-press-haptic` (`selection`,
`impact-light|medium|heavy|soft|rigid`) plays at touch-down without a round
trip, as `-exact-press-scale` does. `haptic("selection" | "impact-…" | "success" |
"warning" | "error")` is a host command an action runs, for example when a
drag crosses a threshold. iOS uses the feedback generators; the web vibrates
where it can; Linux does nothing.

An interface size setting is `rem` plus `setRootFontSize(px)`, CSS's `:root {
font-size }` (LLP 1069.000 D3). Size what should scale in `rem` (text, control
heights, paddings) and what should not in `px`; an action calling
`setRootFontSize(size)` re-lays every `rem` out in its own commit, on every host.
The app's size stands over the host's (the browser's setting, iOS Dynamic Type's
body, 13 on macOS, 16 on Linux), as an author's `html { font-size: 20px }` stands over a
browser's font-size setting; `setRootFontSize("medium")` hands it back, so a
"Default" choice that follows Dynamic Type calls that. Smaller and Larger that
should scale Dynamic Type instead of replacing it are percentages:
`setRootFontSize("87.5%")`, `("125%")`, CSS's percentage of `medium`, so the root
is that share of the host's size and follows it when Dynamic Type or the browser's
setting changes. A size of 0 or less is refused (a literal at compile time, a computed one in `logs`). It is not kept
across a launch: a root `task restore mount` with `after(1, applySize)` sets the
stored size again. Do not multiply a scale factor into every size instead.

Platform facts are reserved sources (`exactViewport`, `exactPage`, `exactDelivery`,
`exactSurface`, `exactTime`); the bake refuses a declared field the source does
not have. `exactViewport().displayMode` is a string: the current CSS display
mode on the web, `fullscreen`, `window-controls-overlay` (the tall title row
`display_override` asks for) or `standalone` on macOS, `standalone` on other
native hosts. Declare it on the resource's shape; `state.device.displayMode`
reports it too. `prefer display-mode fullscreen` substitutes it on Apple;
browser drivers refuse that override, so test real fullscreen transitions. This reads the window, not a fullscreen navigation route.
`exactViewport().pointer` and `.hover` are CSS's `pointer` (`fine`, `coarse`,
`none`) and `hover` (`hover`, `none`) for the primary input: the web's media
queries; `fine` and `hover` on macOS, Linux and Windows, and `coarse` and `none`
on Android's touch screen and on a Linux one with no mouse or touchpad, read once
at launch; on iOS `coarse` and `none`, or `fine` and
`hover` while an iPad has a trackpad or mouse attached; `none` and `none` on tvOS
and on Android TV.
They change while the app runs. Show a game's on-screen stick under `coarse`
(the starter does, `game/recipes/touch-controls.md`), never by platform. The web's
neutral look sizes its own controls for a touch device instead (`any-pointer:
coarse`: any coarse input, normally a touch screen), so they keep their size
while an iPad's trackpad comes and goes. Under the agent each is the device
class's from launch; `prefer pointer coarse` and
`prefer hover none` substitute them. Chrome emulates a touch screen, so it takes
`coarse` with `none` and `fine` with `hover`, and refuses `pointer none`; the
Firefox and WebKit carriers refuse both.
`exactViewport().accessibilityTextSize` is true while the person's text size is one
of iOS's accessibility sizes (AX1 to AX5, UIKit's `isAccessibilityCategory`), where
UIKit's own views drop decoration and stack what sat side by side; it is an Exact
fact, not a CSS media feature, read wherever Exact draws with UIKit's metrics (iOS,
iPadOS, tvOS, visionOS, watchOS), and false on the Mac, Android, Linux, Windows and
the web.
`EmptyState` drops its symbol then. Under the agent on iOS, `prefer root-font-size
28` (Dynamic Type's AX1 body) or more sets it.
`exactViewport().gpuTier` is `low`, `mid` or `high`: how much GPU work a frame can
afford, chosen once before the GPU device is made from what the adapter reports
(a phone GPU of the Adreno 600 class is `low`), else from the screen (a phone-sized
one `low`, a computer's `high`), never from a measured frame time; a phone-sized
screen is never `high`, nor is any iOS or Android device natively. A game's renderer follows it (`Game::quality`); a HUD can
read it to offer a quality setting. A native host answers its GPU module's tier
once the module loads (`high` before it). Under the agent it is `high` from launch, and
`prefer gpu-tier low` substitutes it and remakes each world's renderer at its rung.
`exactPage` answers `visibilityState`, `onLine`, `canShare`,
`canShareFiles` and `canSharePictures` (whether `share(files=…)` takes
`app:/` files and an element's `"#id"`; the web never takes a picture),
`canOpenFiles` and `hasFocus` (`document.hasFocus()`: the app's window has the
system's focus; false while another app or window is in front, so an app can
choose an in-window message over a system notification), `persisted` and
`restores` (the back/forward cache: `persisted` is true while the page is put
away, from a `pagehide` with `persisted` until the `pageshow`; `restores`
counts those `pageshow` events, so `task t when page.restores > 0
key=page.restores` runs once for each return, never at launch; a Mac window a
`keepRunning` close hid is put away, a minimized one only hidden). A reminder that should
open a route uses `showNotification(title="Done", navigate="/thread/42")`;
its click reaches the navigation root as a warm `navigate` event or a cold
launch location. Under the agent `state.notifications` lists `navigate`, and
`type <navigation root> "/thread/42"` checks the destination. Under the agent each
is the drive's (`prefer has-focus false`; `state.device`). Use dimensions,
media preferences, page facts,
and capability state rather than suffixing files by platform. Preference facts
inform authored policy; the engine does not automatically remove all motion.

`exactSurface(name)` is the host's channel for read-only facts about where
the app is running, not only for a GPU surface's published record. The
terminal host publishes `exactSurface("terminal")` (`mode`, `images`,
`colors`, `log`, which counts the `role="log"` nodes it has seen, and
`printed`, the `id` of that log's last child it wrote to scrollback), and an inline app retires printed entries from it (LLP 1101.001
P1; `apps/harness/terminal.contract` shows the task). Declare its shape
(`resource terminal = exactSurface("terminal") as shape Terminal`); on any
other host it stays unloaded. A new host fact uses this channel before
anyone proposes a new reserved source (LLP 1101.002 §0 P11).

The terminal's `colors` is the selected output bit depth: `0` (monochrome),
`4` (16 ANSI colours), `8` (256 colours), or `24` (RGB). The host reduces
authored colours to that palette; at zero it emits no foreground/background
colour codes, while preserving bold, underline, italic and reverse-video
focus. Images use alternate text in monochrome. Authors can choose different
content, symbols or emphasis rather than relying on colour alone:

```contract
shape Terminal
  colors: number
component App
  resource terminal = exactSurface("terminal") as shape Terminal
  view
    column
      if terminal.colors == 0
        text "! Attention required" font-weight=700
      else
        text "Attention required" color="#d78700"
```

Interactive and headless CLI runs detect the palette from `TERM` and
`COLORTERM`. Nonempty `NO_COLOR` disables colour (including `NO_COLOR=0`);
empty `NO_COLOR` does not. `TERM=dumb`, `unknown`, an absent terminal type
without a colour hint, and unrecognized terminal types fall back to monochrome.
This is conservative environment detection, not a terminal capability query.
Use `--color auto|never|16|256|truecolor` on any terminal app or `exact-terminal`
to override it (`always` means `truecolor`; `--color=never` also works).
An explicit choice overrides `NO_COLOR`. Plain `print`/`document` output is
always text; the selected palette also applies to `.ans` screenshots. A Rust
embedder may call `Host::set_colors(ColorDepth::Monochrome)` before starting
the loop or emulator; direct painting and `Vt` otherwise default to RGB for
deterministic tests.

A `text` inside a paragraph can carry `press=` (a `span`'s `onclick`) and
`href=` (an `a`). In the terminal a click on the run runs its
handler; without one (and on every host for `href`) it follows the link: a path naming one of the app's routes is `navigate` on the
navigation root, and an `http`, `https`, `mailto` or `tel` URL leaves the app,
as `openURL` does. An app without routes that wants a run to act in the app
(the LLP reader's cross-references) gives the run `press=`. In a terminal the
mouse is the app's only full screen or while a dialog is open; inline, ⌘-click
on the run's link is the terminal's own.

Localized strings use `t("key", name=value)` and app `strings/<locale>.json`
files. Compile against the files to check keys and placeholders. Formatting
functions accept a narrow set of literal formats; app wording is an app `fn`.

#### Folds on iOS

`exactViewport().devicePosture` and the viewport segments are the web's
([foldables](recipes/foldables.md)). On the iOS 27.1 simulator UIKit moves an alert
(an `alertdialog` with text) on either fold, and a page or form sheet (a sheet route
or a modal `dialog`) on a book fold: folded like a book, an alert to the middle of
the left segment and a page or form sheet to that segment, its right edge about 12
points into the band (measured in left-to-right text); folded like a laptop, an
alert to the bottom segment, while a page or form sheet still spans the fold. The
tab bar stays clear of either fold. [Apple's adaptive-layout
talk](https://developer.apple.com/videos/play/tech-talks/111463/) says instead that
alerts move to the trailing side of a book fold and can move to the top of a laptop
fold, and that action sheets, menus and popovers move clear too, which was not
measured.

A route moved off a window edge is still padded as if it reached it: its top on a
laptop fold, its right on a book fold's segment 0.

## Inspection and testing

On Apple development builds, an `iframe`'s web content is inspectable from
Safari's Develop menu. Enable Safari's web developer features, launch the app,
and select its web view under Develop. This inspects the embedded page; use the
agent operations below for Exact's native tree. Production builds
(`EXACT_UPDATE_TRUST=production`), `exact release`, and IPA archives leave
web-view inspection disabled.

An Apple build's dev menu (a four-finger tap, or ⌘D on a hardware keyboard or a simulator's; on a Mac, Develop › App Info…, ⌘D, in a development build: a Mac build with production trust, `EXACT_UPDATE_TRUST=production`, or made by `exact release` has no Develop menu and takes none of its chords unless it is built or launched with `EXACT_DEV_MENU=1`, or a person runs `defaults write <bundle id> ExactDevMenu -bool YES` and the build did not say `0`; an iOS build, a release one too, has the menu unless it is built or launched with `EXACT_DEV_MENU=0`, LLP 1115 D9) opens with the
build it is in: the app and version, when, where and with which Xcode it was
built, the exact2 commit and the app's own (with branch and a dirty flag), the
build kind, the device, and the app's release notes. `host/apple/build.mjs`
stamps these into the bundle's Info.plist on every build, `--archive` included,
so a shipped binary carries the build machine's host name and the commits too.
Release notes are `release-notes.md` beside `app.contract`: UTF-8 text, shown as
written, up to 16 KB; no file, no section. A deploy script that publishes the
build under a revision sets `EXACT_DISTRIBUTION_REVISION` for the build to show
it. Copy (iOS and macOS) takes all of it as text.

Build diagnostics include stable ids and original file ranges. Locations are
1-based line/byte-column coordinates, with exclusive end columns; a usage, I/O or
manifest error has no range (line and columns 0). Honor related
locations when an error crosses a child prop, injected action, or imported file.
Do not use a character index as a byte offset in Unicode source.

```sh
bun exact.mjs contract build app.contract --json
bun exact.mjs contract symbols app.contract --name save
bun exact.mjs contract build app.contract -o /tmp/app.plan --map
```

The source map (`<plan>.map.json`, beside the plan) is separate from the plan and
keyed by its digest. Use it only for
the accepted generation it describes. The driver can connect layout to the
original declaration, component call sites, and winning style attribute.
Production artifacts do not need a development source map.

The driver has ten operations: `tree`, `screenshot`, `tap`, `type`, `state`,
`layout`, `logs`, `clock`, `prefer`, and `perf`. Variations are arguments, not new
commands; `prefer` takes CSS's media feature names (`"prefer prefers-color-scheme dark"`,
`"prefer prefers-reduced-motion reduce"`). The display's scale, which a hairline is one
device pixel of, is the device's: a simulator's or the Mac's own, and on the web and the
Linux host what `EXACT_SCALE=2` launches them at (1 otherwise; `"prefer display-scale 2"`
changes it under a native host, and `layout` shows the boxes that follow). Targets are `testId`s (or view ids): give every control a `testId` and
drive it on every host, iOS included (`agent ios`), never by screen coordinates.
On a machine with several booted iPhone simulators, name yours: `EXACT_SIM=<udid>`
for `agent ios` and the smokes, `--sim <udid>` for `host/apple/build.mjs`; a
choice among several is refused rather than guessed (another drive's simulator).
A target no `testId` carries resolves by a view's exact accessibility label or
text (`tap "Save draft"`); a name several views share refuses, naming them.
`type` also sets a control's value: `type "persona" "bob"` chooses a
`select`'s option by its `value`, and a date, time, range or checkbox takes
its value the same way, in a drive or a test (a tap does not open a native
`select`'s menu under the driver).
`tree --ax` prints the platform's accessibility tree, as VoiceOver would read it.
Use `tree` to find targets, `state` for data and delivery, `layout` for
geometry, `perf` for the work a drive cost (`perf <target> during "<op>" …`: per
plan site, evaluations, unchanged results, instances created and retired), and
screenshots for rendered output. `state.tasks` gives each task's next due time
(`null` while a gated task is idle or an `after` is spent) and `state.queued` each
queue mutation's waiting sends. Logs name refused operations and data errors.
Under the driver's clock no frame is presented, so `perf frames` measures a
live window instead: `perf frames live 3000` lets the page's clock follow the
wall for 3 s and reports the frames it presented (p50/p95/p99, late frames)
and each game world's frame, tick, feed and encode times, on the web's wasm
target (a game's). The world ticks on the wall in that window, so a hash
taken after it is not a seeked run's.
For a game canvas, JavaScript `s.tap("world", {mouse:true, at:[x,y]})` sends one
primary mouse click on web, macOS, Windows, and Linux (on macOS any node takes
it, so a click can land on a link inside a paragraph). `{contextmenu:true, at:[x,y]}`
sends a right-click. Coordinates are relative to the target's top-left; omit
`at` for its center. Both refuse invalid, covered, or offscreen points and held
contacts. The CLI forms are `tap world mouse [at <x> <y>]` and `tap world contextmenu
[at <x> <y>]`. `tap <target> auxclick` is the middle button and `clicks 3` a triple
click (each press counting 1, 2, 3); every click form and a wheel take `at <x> <y>` and
`modifiers Shift+Meta`, and `down … modifiers Shift` or `drag … modifiers Shift` holds
them to the lift. Chrome and macOS deliver these as a hand's (on macOS through the
application, so its local event monitors see them); iOS, Linux, Windows, Firefox and
WebKit answer `delivery: "unsupported"`, and a word a form does not use is refused by
name. `tap stage wheel 0 -20 modifiers Control`
is a pinch's wheel; `tap world drop a.board` drags a file in (web, macOS). Plain canvas taps and held contacts are
fingers, so their platform pointer identity and retained press history can differ
from a mouse's; use the intended physical input when comparing game saves.

`tap` and `type` scroll a target whose middle is out of view into it first (its
nearest scroll containers, then the page) and say so in the reply's `scrolled`.
A tap aims at the target's middle, or, where the target is not there (a wrapped
inline run, whose middle can fall between its lines), at the middle of the first
of its lines that is; a tap whose point lands on something else fails, an
ancestor that would take the press itself included.
A tap that names a node presses that node, never a control inside it: a node
with its own `press` (or a link) whose middle holds another — a post row's link
card, its Like — is pressed at the nearest point of its box the search finds
that reaches it (the reply's `avoided` names what the middle holds), or refused
when it finds none; a node without one is refused when its middle holds a control
(`tap post-0 would press card-0 inside it; tap card-0, or tap post-0 at <x> <y>`).
The search tries a grid of about 12 points, then about 3 points and the box's
edges 1 point in, so a reachable strip thinner than that, away from the edges,
can be missed: then name a point. `tap <target> at <x> <y>` presses at a point
from the target's top left, whatever a finger there reaches, on every host. What
a press there would deliver counts as a control: a node with its own press, an
SVG element, an inline run or link, a form control or text field (it toggles,
opens or takes the focus), a grouped list's row, a surface's action button and a
canvas that takes input. A disabled node presses nothing (on the web only a
disabled form control: `disabled` on a box means nothing there). On iOS without
`--touch platform` a refused tap has still dismissed, or begun to open, a
painted popover, as the tap's first step.
`type` on a control sets it as a person choosing would, with `input` then
`change`: a `select` takes an option's value or its label, a date, time or
`datetime-local` input its HTML value (`2026-10-09`, `14:00`,
`2026-10-09T14:30`), a range a number, a checkbox `true` or `false`, a radio
`true` (it is unchecked only by checking another of its group). A
`select`, a range and a date, time or `datetime-local` input keep the person's
choice until their bound `value` changes, as a text field does (LLP 1069.001 D4,
amended): an action that writes nothing, or only sends a mutation, shows the
choice until the bound value moves, on every host, and `type` replies with what
the control shows. A checkbox still snaps back to its bound `checked`.
On a text field or textarea `type` inserts the text, one `input` (in a CLI op the
text is the rest of the op's words, joined by one space, quotes included:
`"type new-task buy milk"`;
in a test file each value is one quoted string, a checkbox's `"true"` too); `change` comes as a
person's would, when the field commits (`type <id> key Enter` on a field, or the
focus leaving it), so an edit saved on `change` needs one of those (on macOS a
`tap` on a button leaves the focus in the field, and commits the field's `change`
before the button's `press` runs). A CLI drive's
input does not wait for a request its action sends; `clock data` lands it.
`tap <target> drag <dx> <dy> … during "<op>" …` runs the quoted reads after the
move, with the finger still down, before the hold. Under `--touch platform` the
lift is scripted, so the reads run inside the hold and `during` needs one
(`hold 300 during "state"`); the hold then moves the clock by its length on real time,
as a `clock +<ms> real` would, so the app's `every` and `after` run while it is held. `tap <target> drag to <other> [at <x> <y>]`
ends the drag on another node (refused by name when it is not mounted or off
screen); a drag that should autoscroll a list or a board is `drag dx dy hold ms`. On iOS
without `--touch platform` a drag scrolls the scroll or list it starts on with the
finger, no fling; any other drag there is refused, naming the flag.
On an iOS simulator, append `then dx dy [over ms] [hold ms] [during "op" …]`
to script another leg with the same finger. For example:
`tap handle drag 0 120 over 300 hold 500 during "layout" then 0 -120 over 300 hold 500 during "state"`.
Each delta starts at the previous endpoint. All legs play in one XCTest
synthesizer record, with one down and one up. Reads run inside their leg's hold;
if they outlast it, the drive fails. The complete path must stay inside the
viewport, last at most 10 seconds, and spend at most 2 seconds moving, across
at most eight legs. Library calls use `drag.path: [{dx, dy, over, hold, during}]`
for the additional legs, with `during` read functions. Separate requests never
keep an iOS contact down.
Under `--touch platform` a tap replies `viewGone: true` when its view left the
window before the finger lifted, so that view had no lift (UIKit rebuilds a
grouped row's detail button in the first seconds after the list appears), and
`downAfterLift: true` when the window had the down only after the finger lifted
(the app busy, or the system slow to deliver it; a UIKit switch in a list cell
takes a down and a lift that arrive together as no tap). Read the state, and tap
again if nothing changed; `touch.dispatched` less `touch.ended` says how late
the down was, and an app that blocks its main thread that long loses a
person's tap too.

#### Drags on iOS

A wheel (`tap <scroller> wheel 0 300`) and, without `--touch platform`, a drag on
the scroll or list it starts on (`tap <scroller> drag 0 -300`, past the screen
too) leave it as a person's scroll by that distance does, with no glide: the
content moves that far, a large title collapses as it goes, and a scroll back to
the top opens it again. As a person's, none stops with the content partway under
a large title: it eases to the title's open or closed rest, whichever is nearer,
so under one a wheel of 20 from the top moves nothing (with a search field in the
bar it stays where it ends: not measured). Under `--touch platform` a drag whose ends
are both on screen is a real one (`delivery: platform`): the content follows the
finger once UIKit's pan begins, some 10 points in. Without a hold it glides on
after the lift, and `clock settle` waits up to 4 s for it to stop (`reason:
"scroll"` past that). Held for `hold 100` or longer it stops with the finger,
which the runner steps a point across the drag and back in the hold's last
150 ms, as a finger does (measured along a list); `hold 50` still glides. A
drag that ends past the screen is the agent's drag above, `delivery:
recognized`.
During a reorder `state.reorder` reads `{ item, from, to, before, phase,
ending }`; a grouped grip also takes `type <grip> key Space` and the arrows. `clock +N` moves the virtual clock without
waiting for a store's or the network's reply on real time (unless a timer fires
first); its reply says what is still in flight (`inflight`, on every host), and `clock settle` lands it.
A request the server holds, marked `exactHeld: true` (a long poll, LLP 1016.000 D9), is the exception: no
clock waits on it, neither a jump nor `clock settle` nor `clock data`, and each of their replies names it
(`held`, `heldTargets`; `state.pending` marks it `held: true`). Its answer must be safe to supersede.
After an action that causes a push you will read (a write the server echoes, a build's report), say so with
`clock held [<target>…] [<n>s]`: real time passes with the clock frozen, so no timer fires, until each held
request in flight (or each named) has had one reply, the server's own timeout answer included; then `clock
data` lands what that reply started. Past its bound (20 s) it answers `settled: false, reason: "held"`,
naming the targets that had none.
`clock settle` also runs the clock to where the last running finite, unpaused
transition or animation ends, firing the timers due on the way, so a test on a timer's grid moves with it,
and on past a task due a commit later (`after(1, …)`, which an input or an answer armed) until none is.
`clock data` lands it without moving the clock: the data module's activation and
every request in flight, each answer's `then` with it, no timer fired (its reply names a task
due a commit later, which a tap next would come before: `clock settle` runs it). A CLI drive's
first operation runs at boot and may come before that has landed (an authored test
lands it before its first step), so a drive that reads or taps data starts with `clock data`
(a `tree`, `layout` or `screenshot` taken before the clock first moves, with a request
in flight, says so on stderr).
A playing `video` or `audio` is on real time too: the clock never seeks or holds it, so
between operations it moves only as far as the drive took. `clock +N real` lets
N ms of real time pass with the clock moving beside it, a step at a time: a
video plays that far (its `timeupdate`s arrive), and a reply that lands in the
span lands (LLP 1042 §3).
`clock +N real until <path>` is a test harness's affordance, not something an app
drive needs: the same real time with the clock beside it, ended early once the
file at `<path>` (a local path, not a URL or a glob; its directory must exist)
exists, which the harness driving the drive writes at the event it waits for. The
clock moves by exactly the real time gone, and the reply says `until: "met"` (at
once, `real: 0`, when the file is there already) or `until: "bound"` after N ms.
Under the driver a picker, an export, a share or an auth session opens no panel:
it is held and `state` lists it under `pending` with its capability. Answer it by
the node its answer arrives at (the `id` the command names) or by its capability:
`type @folder-input path/to/folder`, `type @pick photo.jpg`, `tap @open-directory
cancel`. `@N`, its ticket, works too, but a ticket counts every request before it,
so it differs between hosts and runs. On Linux, Windows and Android a tap on a date
or time control holds its picker the same way, by the control's kind: `type @date
2026-10-23`, `type @time 09:30`, `tap @date clear`, `tap @date cancel`.

Authored tests are a smaller language over that API:

```contract-test
test "an action uses its computed next value"
  expect state count == 0
  tap "advance"
  expect text "result" == "1/2"
  expect state doubled == 2
```

This test goes with the complete example above. A test opens with its launch
lines — `size 1200x800`, `epoch "2026-09-21T12:00:00Z"`, `time-zone
"America/New_York"`, `locale "fr-FR"`, `seed 7`, the driver's flags of those
names, and `fresh` (the Snapback dev server emptied first, `fresh
"seed.json"` seeded from a file; snapback4/README.md) — written first in the
test or, for every test, at the top of the file.

`prefer prefers-color-scheme dark` sets a media preference before the first
observation. It can lead the test or the file, or appear later to change the
preference between steps. Use the driver's CSS media feature names and values
(`prefers-color-scheme light|dark`, `prefers-reduced-motion reduce|no-preference`,
and the other media preferences listed above). A test's leading value overrides
its file's default for that name only. Duplicate names at file level or among a
test's leading launch lines are errors; later steps may change the same name.
A bad name, value or unsupported emulation fails that test at its line.
`reload` and verify's relaunch keep the latest applied preferences. On native
hosts they are applied after connection, before the test observes the app;
they do not set its appearance before mount. MIDI's `prefer midi.…` remains a
step inside a test.

Before the first step (and after a `reload`) the driver waits for the app's data
as `clock data` does: its data module activated and every request launch started
(a store's open, a fetch) answered, the clock unmoved and no timer fired, so a
test does not start with `clock settle`. `before data`, a launch line, skips the
wait; what has landed then is the host's (a native app runs on real time before
the driver connects).

To test an error path, fail the fetch: `fail fetch "<url prefix>"` makes every
later fetch whose URL starts with it fail exactly as a refused connection does
on that host (a TypeScript source's `fetch` rejects with `FetchError` kind
`"Network"`; a Rust source's request settles `Failed { kind: Network }`), and it
never goes out. Leading the test it is armed before the first data load, so
"the API is down when the screen opens" is the launch; later it is a step.
To test going offline after the data loaded (a Snapback4 partition, which
cannot open before its first sync, is the usual case), put `clock data` first;
a runner note names a `fail fetch` armed before the data loaded.
`times N` fails only the next N; `pass fetch "<prefix>"` stops it; a counted
fault that never fired fails the test. A fault reaches only fetches that start
after it: a request already in flight with the same arguments is reused by the
runner (one request per resource and arguments), so re-opening the screen while
it is pending gets that request's answer, not the fault. Arm the fault before
the request starts (as the launch line, or before the step that first loads
it), or let the pending one settle (`clock data`) first. The app's own `catch`, error record and
retry run, so this checks the real error handling (LLP 1103). A drive takes
`--fail-fetch <prefix>` at open and the ops `"fail fetch <prefix> [times N]"`
and `"pass fetch <prefix>"`; `state.faults` shows each prefix's hits.

```contract-test
test "the list shows an error, then retries and loads"
  fail fetch "https://api.example.com/recipes"
  expect text "error" == "Couldn't load recipes."
  pass fetch "https://api.example.com/recipes"
  tap "retry"
  clock data
  expect tree has "recipes"
```

`fail fetch` tests a rejected request; it cannot supply an HTTP status or body.
To test malformed successful responses or a specific HTTP error, make the app's
test server return that fixture and drive the real source against it. Do not
invent a response-body override in Contract test syntax.

To write through the app's own server as another person, `request POST "/items"
{"title": "Bread"} as grace` is the driver calling it (`Authorization: Bearer grace`,
under app.ts's local URL; a status outside 2xx fails the step with the status and
body, unless `… as bob expect 409` says it is the refusal the test wants). The app shows the write when it next reads: `clock +<its poll's ms>`, `clock
data`, then `expect` ([rest-backend](recipes/rest-backend.md),
[authored tests](contract-grammar.md#authored-tests)).

A slow server is bounded in the source, not the view: `fetch(url, {
exactTimeout: 10000 })` cancels the exchange after 10 s (headers and body) and
rejects with a `FetchError` of kind `"Timeout"`, which the source catches and
answers as any failure (a Rust source's request takes `Request::timeout(ms)`).
Without it a stalled fetch waits the platform's limit (60 s without data on
Apple). Test the error state with `fail fetch`, as above; a timeout itself is
tested against a stand-in server that never answers (the reference's
"exactTimeout").

A test whose text depends on the date names its `epoch`; without one it runs at
the driver's 2026-01-01 UTC. A drive or test whose app talks to a live backend
(`snapback4 dev` runs on real time) says `--epoch now` or `epoch now`: the
machine's clock, read once at launch, from which `clock` moves the date as
before, so the app's dates agree with the server's but differ run to run. The steps are `tap "id" [hover|dblclick|contextmenu]`,
`tap "id" modifiers "Shift+Meta"` (a press with keys held),
`tap "list" into "key"` (a virtualized list's row brought into view by its key,
so the next step can tap a row outside the rendered window),
`tap "id" pinch <scale> [at x y]` (two fingers; `scale` greater than 0),
`tap "id" drag dx dy [from x y] [mouse] [press ms] [over ms] [hold ms] [during "op" …]`
(a finger, `pointerType` touch, where the carrier has one, unless `mouse` names the left button; `during`
is last: quoted reads or `clock` while that contact is down, after the move and
before the hold; `press` and `hold` advance the virtual clock, except under
`--timing platform`; on an iOS simulator it is a real UIKit gesture, so a swipe row's
full swipe performs its action),
`tap "id" drag to "other" [at x y] […]` (it ends on the other node's middle, or
at a point in its box; a card dropped on another list; on iOS a `tap` naming a
native swipe action's control performs it as assistive technology does, with no
swipe, where the web's tap scrolls the row to it, and `--touch platform`, as in
`bun exact.mjs test ios --touch platform`, makes every tap a real touch),
`type "id" "text"` (sets the value), `type "id" "text" append` (after the value
the tree shows, as typing after a prefill), or `type "id" key "Name"`
(`down`, `up` — its `keyup` handlers hear it — or `for <ms>` on the virtual
clock, repeating as a held key does: a keydown with `repeat` true 500 ms after
the down, then every 83 ms),
`type "id" paste "text"` (⌘V on macOS, Ctrl+V elsewhere, then the paste; a `key` handler that `preventDefault()`s that chord keeps it from landing), `type "id" copy`, `type "id" cut`, `pick "id" "path"…` or
`pick "id" cancel` (a held picker or export, by its node or capability as
above; paths are the test file's), `clock settle|data|+ms|+ms real|ms` (`data`:
what is in flight lands, with each answer's `then`, the clock unmoved), `clock held [<resource>…] [<n>s|<n>ms]`
(the driver's: real time with the clock frozen until each held request, or each named, has had a reply, such as a
long poll's after a push the test caused, within the bound, 20 s unless given; then `clock data`), `resize
800x600` (the window, mid-test), `reload`
(the app restarts on the store it had, including a `secret.keep`, its state and
clock starting over, so a test shows what persists; the web reloads its current
URL, including its agent launch facts; a native app reopens at its launch
location), `close` (the window's close button, as ⌘W: a `beforeunload` that
calls `preventDefault()` keeps it open and the test goes on to the app's "Save
changes?"; macOS and the web), `back` (the platform's Back: a dialog's cancel,
a sheet's dismissal, the pushed route popped; a refusal is a note, nothing to
go back from fails),
`screenshot "file"`, `expect tree has|missing "id"`, `expect disabled|enabled "id"` (a control's
`disabled`; a `tap` on a disabled one fails its step on every host), `expect text "id" == "…"`
(the node's text; a control's value, so a `select` reads its chosen value, not its
options, and a checkbox with a `checked` binding `true` or `false`; else its descendants' — a button's label — else a field's value), and
`expect state name == <number|string|bool|none|[]>`, where `name` may go on into
a record's fields (`board.active.present`) or a list index (`rows.0`), and the
number may be negative (`== -3`). `expect wakelock held|released` reads
`state.wakelock.held`: the runner's record of displayed elements requesting the
lock, not whether the OS or browser granted it. A mismatch names the holding
view ids and their testIds when present, or says nothing holds it.
A failed expect names any resource that
failed, and why; with no input before it, the requests still in flight (the boot's
own, or what a `clock +N` left on real time). An input step ends with what it settled: an answer the data
module gave in the input's turn, and its mutation's `then`, are there for the
next step. Otherwise the clock stands still between steps: a reply on real time
(a store's, the network's) or a transition an input started lands at a `clock`
step, so `clock settle` before the `expect` that depends on it. Storage work that
has answered by the end of a web input may still be pending on iOS, macOS or
Linux: before a later step depends on its reply or its mutation's `then`, use
`clock data` (or `clock settle`, if motion should finish too). A timer
(`after`, `every(ms)`) fires when the clock reaches or passes its time: `clock
settle` fires it only if it reaches it while advancing to a motion's end, so
move to it with `clock +N` (a `task … after(1, restore)` needs `clock +1`).
`type "id" key "Name"`
focuses the target if it takes the focus (else leaves the focus where it is)
and presses the key as a keyboard would on every host: its `key` handlers,
then its default — `"7"` types into a field, `"Enter"` submits it (a
textarea's breaks the line), `"Space"` presses a button, `"r"` reaches an
`aria-keyshortcuts="r"` button — then releases it through the `keyup`
handlers at the focus. A chord holds its modifiers for the key, in
Playwright's spelling: `"Shift+Enter"`, `"Meta+s"`, `"Control+Alt+ArrowLeft"`
([keys](contract-grammar.md#keys)). As a keyboard does, on every host, each
modifier is its own key first: a `key` handler hears `Shift`, then `Enter`
(with its `shiftKey`), so a handler that treats any key as typing must skip
`Shift`, `Control`, `Alt` and `Meta`. A paste is the same (`Control` or `Meta`,
then `v`), and so is a click or a wheel with `tap … modifiers` on Chrome,
Firefox, WebKit and Linux, and a right or double click with modifiers on Chrome.
Each modifier comes up after the key, in reverse, without its own bit, so a
`keyup` handler hears it too. Still flags only, with no modifier key of their
own: Apple's taps, a drag's phased `tap … down`/`move`/`up` (Chrome and Apple;
other carriers refuse modifiers there), and a key a canvas world takes on Apple.
Not every interactive
driver operation is a test-file statement. `contract test` parses and prints JSON;
`agent.mjs <host> --test <file>` actually drives the app.

Test the behavior being changed: a filter retaining row identity, a text edit
sending the new value, an async error retaining its placeholder, a route back
restoring a screen, or an animation at a specific virtual time. An assertion
that restates a constant is weaker evidence than the user's actual sequence.

## Repair common mistakes

| Refused or misleading form | Accepted direction |
| --- | --- |
| `action save writes item` | Delete the clause; effects come from the body |
| `transform` on a box, inline or through `class`, beyond `translate`, `rotate` and `scale` in that order (`lower-attr-tag`) | A box `transform` that is those functions, each at most once, in CSS's individual-property order (translate, then rotate, then scale) lowers to the rows it equals: `transform="translate(0px, 25px) scale(0.8)"` is `translate="0px 25px" scale=0.8`; `none` is no row. The rest (`matrix`, skews, `scale(x, y)` with two factors, 3D functions, a function repeated or out of order, a bound string) is deferred (LLP 1055.000 D5): write the rows. A `transform` beside a `translate`, `rotate` or `scale` row on the same element is refused; keep one. SVG elements keep the whole property. |
| Component `contract` assertions | Authored `test` blocks exercised by the driver |
| `provide theme = value` around view children | Component `provide` section, wrapper component if needed |
| `items[0]` | `first(items)` or `items.at(0)`, an option to `match` |
| `items.map(x => Row(...))` | Keyed `for` with `Row(...)` in its body |
| `{ title: value }` | Declared record constructor |
| `[...xs, x]` / `xs.push(x)` | `xs.concat([x])` / the same, assigned: `xs = xs.concat([x])` |
| `{...old, title: value}` | `Shape(old, title=value)`, with the record's declared shape |
| `if name` for a string | `if name != ""` |
| `selected.title` when optional | Exhaustive `match selected` |
| `press={() => save()}` | `press=save` or `press=save(captured)` |
| Copying an action's statements into another | Call it: `arrive(path)`; the callee runs in the same commit |
| A helper that reads a slot its caller just assigned | Pass the value: `follow(next)` (`analyze-call-stale-read`) |
| `button press=add` with no child, or only an `image "symbol:…"` (`lower-button-unlabelled`) | Its words in it (`text "Add"`), or `aria-label="Add"` on a symbol-only button |
| A `button` inside a `button`, as a React Native `Pressable` in another (`lower-nested-button`): a press is the inner one's alone, but the outer's accessible name reads both ("Note Listen") | A `row` of the outer's content as one `button` (`flex=1`) and the inner one beside it |
| A heading at the top of a route's content that says its `header` heading again, which shows twice under the iOS bar (`lower-title-twice`) | Drop it; the bar's title names the screen. A heading below names a section: `text "Results" role="heading" aria-level=2` |
| An SVG `use` whose literal `href="#id"` targets a `foreignObject` or a subtree containing one (`lower-svg-use-foreign-object`). Only unconditional literal SVG ids from one source definition resolve. Repeated copies must all contain a `foreignObject`; a conditional copy, a copy without one, ids from different source definitions, dynamic hrefs and dynamic ids stay quiet. References inside the target are not followed | A `foreignObject` draws nothing through `use`. Place it directly in the SVG for the web; native hosts refuse it. For content on every host, position an ordinary box over the SVG |
| A list and its detail (or a form) swapped by `if showingDetail` / `else` or complementary sibling `if`s (also with the same viewport-only `or` term), each with its heading, in an app with no `routes` (`lower-screen-by-state`) | Each screen a route: `routes`, a row that `push`es its detail, `back` to return; the web gets Back, reload and links, a wide window the split (`contract vocab screen`) |
| `column padding=24 width="100%"`, as wide as its parent's content plus the padding: it runs past the screen's right side (`lower-width-padding`) | Drop the width (a block or a stretched flex item fills its parent), or `box-sizing="border-box"` |
| An absolute width ceiling containing scalable text stays bounded at accessibility sizes (`lower-text-px-width`). The lint follows `width` and `max-width` through their bounds: a fixed maximum alone caps growth; a minimum alone does not. A fixed minimum can override a smaller cap, but a growing or unknown minimum makes the lint stay silent. Absolute lengths include numbers, px, pt and absolute `calc()`; `em` is fixed against an authored absolute font. Flex can supersede width on a row’s main axis; zero grow with an auto basis keeps its ceiling, and flex does not size a column child’s width. Effective classes, declaration order and parent direction count. Unresolved classes and uncertain sizing stay silent. A computed display leaves the node uncertain but its children are still checked; known-hidden subtrees are skipped. Text in absolute positioning, popovers and dialogs is checked inside that subtree, without counting against enclosing boxes. Text inherits the platform root font. Native Apple controls use the Apple control font, which can scale with Dynamic Type; web controls inherit, and Linux control defaults are fixed. An absolute font size stops scaling within that boundary; `inherit` and `unset` use the parent font even on controls. The warning reads lowering’s grouped-list typography and checks layout boxes containing text runs or text fields, including HTML inside SVG `foreignObject`; an inline run’s own width does not constrain its paragraph | Express the width and its bounds in `rem`, such as `width="3rem"` or `max-width="3rem"`, so they follow the root font, or let the content determine the width |
| `text "Results" aria-level=2`, a level with no role, which is no heading to a screen reader on any platform, gets no heading style and is not the iOS bar's title (`lower-level-without-heading`) | `text "Results" role="heading" aria-level=2`; `role="heading"` alone is level 2 |
| `column role="dialog" aria-modal=true` for a modal you lay out (`lower-dialog-role`) | A sheet, a route with `navigationPresentation="modal"` ([navigation-stack](recipes/navigation-stack.md)); a confirmation is `dialog role="alertdialog"`, opened with `showModal(id)` |
| `image "symbol:heart" -exact-tint-color="-exact-secondary-label"`, which Apple draws in the text colour: a symbol is drawn in its `color` on every host, and `-exact-tint-color` colours a raster image or a native button's face (`lower-symbol-tint`) | `image "symbol:heart" color="-exact-secondary-label"` |
| `` `${n} cards` ``, which reads "1 cards" (`lower-count-plural`) | The noun chosen by the number: `` `${n} ${n == 1 ? "card" : "cards"}` `` (`contract vocab plural`) |
| `for p in posts` with `users.filter(u => u.id == p.userId)` in the row: every update walks posts × users, about 100 ms at 1,000 × 1,000 on a Mac (`lower-row-lookup`) | Join it in the data module: the source gives each post the field it reads (`userName`). A `derive` of the join is one evaluation and passes the list bound |
| `RoundButton(label="Start quiz", …)`, cut to "Start q…" in its 80-point circle (`lower-round-button-label`) | The circle is for a word (Start, Stop, Lap); a screen's main action of more words is `button buttonStyle="filled"` with its `text` (`recipes/content-design.md`) |
| `now = performanceNow()`, then `now` read in the same action (`analyze-read-after-assign`) | Bind it and read the local: `let stamp = performanceNow()`, `now = stamp`, then `stamp` |
| Child `resource`, `mutation`, or `task` | Root-owned declaration and props/injections |
| `fontSize`, `radius`, `resizeMode` | `font-size`, `border-radius`, `object-fit` |
| `width=20px` | `width=20` or `width="20px"` |
| Unbounded `scroll` | Real height/max-height or flex in a bounded layout |
| `state item = none` with no usable type | Declare it (`state item: option<Item> = none`), or rethink whether it is mutable state |
| `let next = …` where `next` is already an action, state or other name (`type-let-shadow`) | A name of its own: `let revised = …` |
| `let op = ""`, then `op = "tab"` in a branch (`type-let-reassign`) | Choose the value where it is bound: `let op = key == "Tab" ? "tab" : "insert"`; or a `state` |
| `mutation exported` and `action exported` (`type-duplicate-name`) | One name per declaration: a component's props, states, derives, resources, mutations and actions share names (`action fileSaved`) |
| Dynamic navigation template | `path("route", args…)` |
| Unconditional per-frame app work | CSS/presentation motion where possible; a frame task gated on the state that needs it (`task fly when flying`) |
| An always-on `every` that checks whether a toast expired | `task hide when toast != "" key=toastUntil` with `after(ms, clear)` |
| Add a function because it exists in JavaScript | Check the roster or put the operation in the data module; `len`, `xs.reduce(…)`, `xs.find(…)`, `push(xs, x)` and their kind are refused naming what to write |
| `background-color: "Canvas"` in a `style` | `background-color="Canvas"` (or nothing: an unset background is the platform's) |
| `change=flip(t.id)` on a checkbox, `action flip(id: string)` | The event appends its payload: `action flip(id: string, checked: bool)` (the refusal spells it) |
| Two `send`s to one mutation in one action | One combined request, a mutation per request, or `mutation … queue` to run both in order |

What compiles and then misbehaves (an image tile at its intrinsic size, native bars
the agent does not show, a back gesture refused) is in
[the pitfalls list](agent-pitfalls.md), with what to do about each.

Do not “repair” a refusal by adding a compatibility alias to the compiler. A
new language or host feature is a separate, explicit implementation decision.
Repository repair loops are capped at three rounds; report a remaining blocker
with the diagnostic and evidence rather than silently weakening the result.

## Completion criteria

Before reporting an app change done:

- The intended Contract and imported files compile; required data sources exist.
- Complete examples use current syntax, including inferred effects and provider sections.
- The app was built for every affected runtime surface required by repository policy.
- The requested interaction was driven, with state/log/layout evidence as appropriate.
- A reference design is matched in structure, controls and hierarchy, not pixels:
  native controls set their own metrics, so stop when it reads as the same app.
- Authored tests and relevant checks pass, or remaining failures are named accurately.
- Formatting and line caps hold; no generated artifacts or unrelated work entered the commit.
- The result states what changed, how it was verified, and any actual platform limitation.

For learning, continue with the [human guide](contract-for-humans.md). For exact
syntax and vocabulary, use the [grammar reference](contract-grammar.md). For a
new feature, start from the corresponding compiler fixture rather than memory
of JavaScript, React Native, or the predecessor's Contract language.

CSS overflow accepts `visible`, `hidden`, `clip`, `scroll`, and `auto`.
`clip` clips without a scroll container or the zero automatic minimum size of `hidden`. `auto` clips and
permits scrolling, with indicators shown only when content overflows. A visible
axis beside hidden/scroll/auto computes to auto, as on the web.

`filter` on a box or an `image` takes CSS's functions in order (`blur`,
`brightness`, `contrast`, `drop-shadow`, `grayscale`, `hue-rotate`, `invert`,
`opacity`, `saturate`, `sepia`): `image "logo.png" filter="brightness(0.6)"`
dims a logo. The element and its content are drawn, then filtered (Filter
Effects 1 §12); the web, macOS and iOS draw it, and Linux draws it unfiltered
for now (SVG Gallery's `fx-box-filter` and `fx-image-filter`). `url(#id)`
naming an SVG `filter` works on SVG elements everywhere, but on a box or an
image only on the web: Apple does not take it there yet (LLP 1055.000 §0).

`backdrop-filter="blur(8px) saturate(1.14)"` filters content behind a box;
the two functions may appear in either order. On macOS it samples only the
parent's subtree: the parent's own paint and earlier siblings. Content behind
a clear wrapper can therefore show through sharp. Make the glass a direct
sibling of the content it covers, including a scrolling list. iOS and tvOS
draw the system blur approximation described in LLP 1053.000 §3.

`flex` accepts CSS `none`, `auto`, a basis, or `<grow> [<shrink>] [<basis>]`
(including `flex="0 1 auto"`). Numeric bindings keep the `n 1 0%` meaning.
Shorthands may be literal choices; computed strings are refused. A scroller
with `min-height=0` and positive shrink fits under a bounded flex column.

`display="grid"` takes `grid-template-columns` and `grid-template-rows` as
CSS track lists: px, %, fr, `auto`, `min-content`, `max-content`, named lines,
`fit-content(px or %)`, `minmax()` and `repeat()` (fixed counts, `auto-fill`,
or `auto-fit`). Track lengths also accept `calc()`, `min()`, `max()` and
`clamp()` over px and %, including inside `minmax()` and valid repetitions:
`grid-template-columns="repeat(auto-fill, minmax(min(100%, 200px), 1fr))"`.
Percentages use the grid's content box in that axis; when it is indefinite,
they act as `auto` during intrinsic sizing and resolve against the resulting
size. Negative calculated sizes clamp to zero. The wire and web use equivalent
canonical CSS, so whitespace and simplified expressions may differ from the source.
Calculated `fit-content()` limits, other length units, `var()` and `env()`
remain unsupported here; `grid-auto-columns` and `grid-auto-rows` are not yet
Contract attributes. See [CSS Grid's track sizing](https://www.w3.org/TR/css-grid-2/#track-sizes).

Dimension rows (width/height, their min/max, padding/margins, offsets and
border radii) accept `vw`, `vh`, `vmin`, `vmax`, and `svw/svh/lvw/lvh/dvw/dvh`.
On native, all viewport variants follow the window; on web, CSS resolves
small/large/dynamic viewports. Scalar lengths such as font size and gap do
not yet accept viewport units.

The same rows take `env(safe-area-inset-top|right|bottom|left)` and CSS's
`calc()`, `min()`, `max()` and `clamp()` over px (and in/cm/mm/q/pt/pc/ch/lh),
percentages, those insets, viewport segments and viewport lengths. Functions
can nest, add or subtract lengths, and multiply or divide by a number:
`padding-bottom="clamp(15px, env(safe-area-inset-bottom), 60px)"`,
`bottom="calc(clamp(15px, env(safe-area-inset-bottom), 60px) + 59px)"`,
`width="min(50%, calc(100vw - 2 * env(safe-area-inset-left)))"`.
`clamp()` takes `none` for either bound (`clamp(none, 20px, 50px)`). CSS's
order is `clamp(MIN, VAL, MAX)`, not React Native's `clamp(value, min, max)`.
Native hosts resolve percentages during layout and update calculations when
insets or viewport dimensions change. The web uses CSS functions directly.
The insets are the document's: on the web the browser's, and on a native host the
device's under `viewport-fit="cover"` and 0 under `auto`, even on iOS when a root
`tablist` or navigation stack takes the whole screen; there a box at the root that
pads with them stays under the status bar
([pitfall](agent-pitfalls.md#native-presentation-and-navigation-ios)).
Sizes, padding and radii clamp negative results to zero; margins and insets keep
them. `env(titlebar-area-x|y|width|height)` is the title bar beside a Mac window's
traffic lights under `viewport-fit="cover"` (in the tall title row an app without routes
gets from `"display_override": ["window-controls-overlay"]` in `app.json`) and is undefined
elsewhere, so give it a fallback (`contract vocab titlebar-area`). Unsupported: `rem`/`em`, a unitless number where a length goes
(write `0px`, not `0`), `var()`, more than 32 values or 32 levels (LLP 1001 §2,
"Comparisons").

`translate` takes one or two lengths, each in px or a percentage of the box's own
border box, as CSS's does: `left="50%" top="50%" translate="-50% -50%"` on an
absolute box centres it, a percentage follows the box's size, and transitions and
keyframes interpolate the two parts as CSS does a `calc()`. `calc()` itself is
refused.

Transitions animate translate/scale/rotate/opacity, box paint (color,
background-color, border colors, -exact-tint-color, box-shadow), SVG paint/geometry
and pixel heights. On Apple, numeric `height` transitions work with either
`box-sizing`, without `interpolate-size="allow-keywords"`. An `auto` endpoint
requires that opt-in and `box-sizing="border-box"`; automatic height transitions
require one layout root and no registered content region. `width` and other general layout
properties cannot interpolate yet: native layout is not run per frame.
The diagnostic names this engine limit; `-exact-layout-transition` animates a
change in the laid-out box using the existing measured projection.

`cursor` takes CSS cursor keywords (`pointer`, `grab`, `grabbing`, etc.) and
inherits. Web emits CSS; macOS maps to NSCursor with system artwork stand-ins
where needed; iOS/tvOS/Linux ignore the hint (LLP 1001). URLs are refused.

Omit `font-family` for the platform's font. `font-family` accepts literal CSS fallback lists and choices of them, including
`"Inter, system-ui, sans-serif"` and quoted names. A family declared with
`font` uses its bundled faces; other names are local installed families,
whose own italic and bold faces `font-style` and `font-weight` select by CSS's
matching (a family without the face draws its nearest one, never a synthesized
slant or smear: LLP 1019 §5). Web and Apple retain the ordered glyph fallback cascade. Linux selects the first
installed family, then uses cosmic-text's platform glyph fallback; it logs
this declared limitation for a multi-member stack (LLP 1001).
Under Charlie's 2026-10-11 ruling, a string from state or data may also name
an installed or generic family list, as in `font-family=family`. The JS web
target applies it as CSS; macOS, iOS and Linux register session-scoped stacks
and resolve installed faces when the string changes. An invalid list (an
unquoted name that is not CSS identifiers, such as `Arial;` or `1Arial`: quote
it) and `inherit`, `unset` or `revert` leave the family unset, so the text
inherits its parent's family, as the browser does; `initial` is `system-ui`. Runtime strings do
not load font files.
A runtime choice with a bundled-font literal is refused so it cannot silently
select an installed face with the same name; write bundled choices as literals.

A `text` with `selectionchange=act` hears the reader's real text selection
on the web, macOS and iOS: `action act(s: Selection)` gets `s.text`, `s.start` and
`s.end` in the node's own text ("" at 0, 0 when nothing there is selected).
On web and macOS, `s.rects` is a list of `Geometry` snapshots for the selected
fragments in viewport space; empty when cleared (and on iOS).
`removeAllRanges()` clears the reader selection, and
`setSelectionRange(id, start, end[, direction])` selects a `text` on web and
macOS using source UTF-16 offsets. Both report changed parts through
`selectionchange`; see [text selection](contract-grammar.md#text-selection).
Drive it with `tap <id> drag <dx> <dy> from <x> <y> mouse`; on iOS, where a
long press or a double tap selects a word of text its author made selectable
(`user-select="text"`), with `tap <id> contextmenu [at <x> <y>]` or
`tap <id> dblclick [at <x> <y>]`, whose reply's `selection` says what was
selected (under `--touch platform`, `tap <id> drag 0 0 press 900` is a real
long press).

For a plain text field on the **web, macOS, iOS and Linux**, `setRangeText(id, text[, start,
end[, selectionMode]])` replaces its selection or a UTF-16 range. Selection
is preserved by default (`select`, `start`, `end` are the other modes).
Exact deliberately fires one `input` so its action can update the bound
value, with `InputEvent.inputType="insertReplacementText"`; it adds no undo
step. That input type also covers hatch replacements and browser spelling
corrections, so it does not uniquely identify this command. `select` follows
input only when the selection changes. URL inputs trim surrounding ASCII
whitespace. Markdown editors, Windows, Android and terminal do not support it yet. Field `selectionchange` reports caret moves through `InputEvent`, and
`beforeinput` reports the proposed edit's `inputType` and optional `data`.
Call `preventDefault()` to reject a cancelable edit; composition events
are noncancelable. See the [form controls](contract-grammar.md#form-controls)
section for host limits.
See [form controls](contract-grammar.md#form-controls-radio-inputevent-setselectionrange).

Book typography is CSS's on every host. `text-align="justify"` fills all but
a paragraph's last line. `text-indent` is a length (`text-indent="1.5em"`,
`text-indent=24`; negative with the same `padding-left` hangs the first line).
`hyphens` is `manual` by default: a soft hyphen (U+00AD, written as the character itself or from data) breaks
and shows a hyphen; `none` ignores it; `auto` also hyphenates by the document's
language on the web and Apple (Linux has no dictionary and breaks only at soft
hyphens).

`text-wrap="balance"` (CSS's `text-wrap-style`, inherited) evens a short
paragraph's lines, for a tooltip, a toast or an empty state's message: the
lines before its first forced break take the narrowest width that keeps their
count, as Chrome's do, on every host. It balances up to six lines (more, or a
`line-clamp`, wrap as usual), moves only the breaks (the box measures as
before, and `text-align` places the lines in it), and `auto` turns it off. `pretty` and `stable` are refused; supported values are `auto | balance`.

`text-box` cuts a paragraph's line box to its glyphs (CSS Inline Layout 3 §4),
to centre a label in a pill or line a heading's cap height up with an icon
without padding by eye: `text-box="trim-both cap alphabetic"` trims the first
line down to the cap height and the last up to the baseline. Write the trim
(`none`, `trim-start`, `trim-end`, `trim-both`) and the edges in either order,
or `normal`; the edges are over (`text`, `cap`, `ex`) then under (`text`,
`alphabetic`), and `auto` or a lone `text` sets both (a lone `cap` or `ex` is
refused: CSS's one-word edge also names the under edge). The longhands are
`text-box-trim` and `text-box-edge`. The cut uses the paragraph's own font, as
CSS's root inline box: a smaller run inside does not change it.
`text-box-edge` is inherited, so it may sit on a `view` above; the trim is
not, and goes on the `text`. Exact refuses the trim on a `view` or a `column`
(CSS trims a block's first and last lines, which Exact does not reach yet), on
an inline `text` run, on a `text` that is itself in columns, one included (CSS
trims every column), on a `text` laid out as flex or grid (CSS does not trim
it), and the `ideographic` edges. A later declaration, the node's own over its
class's, overrides an earlier one, and a `?:` choice of literals is refused
only when one of its values would be. A paragraph split across a `view`'s
columns is trimmed at its outer ends only. Every host trims: Apple and Linux
measure and paint the cut, the web writes CSS's own `text-box`.

Multi-column layout is CSS's (LLP 1093). A `view` (or a `text`) with
`column-count`, `column-width` or `columns="12em 3"` flows its content through
columns of one width, separated by `column-gap` (unset, 1em) and an optional
`column-rule="1px solid #ccc"`. `column-fill` is `balance` (the initial value:
columns as even as the content allows) or `auto` (each column filled to the
height in turn). Give the box a `height` and its overflow columns continue
sideways, one per page: that is how a reader pages a chapter, moving the
container by `translate` a page width plus a gap at a time, and reading where
the flow ends with `frame()` of an empty `view` after the last paragraph.
`widows` and `orphans` (2 by default, inherited) keep a paragraph's first and
last lines off a column's edge; `break-before`/`break-after` take `column`,
`avoid`, `avoid-column`; `break-inside` takes `avoid`. On native hosts a box
that has its own height, a background, a border or a shadow, a row flexbox or
a grid, is kept whole in one column where Chrome would split it; the journal
says so once per box. Refused, each saying what to write: `column-span`, page
and region breaks, `balance-all`, dashed or dotted rules, and multi-column rows
on `row` or `column` (CSS ignores them on flex and grid; write `view`).

A text field (`input` of type `text`, `email`, `password`, `search`, `tel`,
`url`, `number` or no type, and `textarea` outside the Markdown editor) is the
platform's own field by default (LLP 1104). On the web it inherits the page's
font, as a CSS reset does; other platforms use their control's own text style.
Its text is its own `color`, else the design's `FieldText`, else the
platform's field ink, on every host: a colour set on an ancestor never tints
it (LLP 1115 D4, LLP 1121 D9). A field of your own (`appearance="none"`) and
the Markdown editor inherit `color` as text does. Disabled and placeholder
appearances are the platform's. To colour
the placeholder, write CSS's `::placeholder` as a block under the field (its
value may compute) or in a `style`; it admits `color` alone, with its own
alpha, and without it the placeholder keeps the platform's colour (LLP 1104
D3; `contract vocab ::placeholder`):

```contract
style Field
  ::placeholder
    color="#6b7280"

component App
  state dark = false
  view
    column
      input placeholder="Search" class=Field
      textarea placeholder="Notes"
        ::placeholder
          color=(dark ? "#9ca3af" : "#6b7280")
```

A pressed, hovered, focused, disabled or checked look of your own is a state
part (LLP 1121 D8): a block of that state's paint rows, in a `style` or under
the node before its children.

```contract
style PostButton
  background-color = "#1083fe"
  :active
    background-color = "#0b5fbd"
  :disabled
    background-color = "#9cc8fb"

component App
  state busy = false
  view
    column class=PostButton press=post disabled=busy
      text "Post"
  action post
    busy = true
```

`:disabled` and `:checked` (`checked`, or `aria-checked`, `aria-selected` or
`aria-pressed` true) are the runner's: they take `background-color`, `color`,
`border-color`, `opacity`, `box-shadow` and `-exact-tint-color`, `:checked`
under `:disabled`, and `:disabled` is your own disabled look (no default dim).
`:active` is the host's own press of the innermost node with a `press`: it takes
`background-color`, `color`, `opacity` and `-exact-press-scale`, which become the
node's pressed paint (LLP 1061 D7), eased on the press curve (under Reduce
Motion too, as the press is feedback) and replacing the host's unsaid press (no
65% dim on top); its `opacity` is the opacity it shows pressed, not a share.
Say each once: `:active background-color` beside `-exact-press-background-color`
is refused (`lower-state-press-paint`), as is a border or shadow there, which the
press cannot paint yet. `:hover` and `:focus-visible` take `background-color`,
`border-color`, `opacity` and `box-shadow`, shown at once while they hold (a
`:hover` replaces the platform's hover wash, as `:active` its press; on
the web a `transition` of the node's own on one of those rows still times it), the
focus's over the hover's, under the press. Each host applies them on its own
tracking, over the node's rows (rows a part doesn't name show the base, also a
base that changes while pressed); their rows are literals, tokens included. A
disabled node takes none. The web (both targets) does this today; Apple, Linux
and Android follow (LLP 1121 stage 8 B). A part
on a native control or a list's row is refused (their states are the
platform's); a node's own part whose state it never writes (`:disabled` with no
`disabled`) is warned of (`lower-state-dead`).

A native control takes `accent-color` and, a field, the design's `Field`,
`FieldText` and `placeholder` roles, where its host honours them: the
schema's control-role table says which, per host
(`docs/control-roles-measured.md`). For each target the app ships (not the
terminal, which draws no colour, nor tvOS or watchOS, which have no column), a
parameter you set that the host there does not honour is warned of once
(`tamber-unreachable: accent-color on macOS switches: NSSwitch draws the
system accent (the first at line 12; LLP 1121 D9)`): the platform's look is
the answer there, or a control of your own (`appearance="none"`, or a styled
pressable for a disabled prominent button). A field's own `color`,
`background-color` (or `background`) or `::placeholder color` wins over the
role, so it is not judged.

A background, border or radius makes it your own box, as in a browser;
`appearance="none"` says so explicitly. This is decided once after classes
and shorthands: a row on any conditional arm counts, even if its value is
`none` on another arm. `background-clip` and `background-attachment` do not
make it bare. `appearance="auto"` explicitly asks for the native field and
refuses background, border and radius rows, naming each longhand. Appearance
is a literal, resolved from the class then the field's own attribute; to
switch it, write `if` with two fields. Excluded input types and the Markdown
editor keep the bare text-input box or their existing specialised control.

A focused field shows the platform's ring: the OS's on a native field, Exact's
on a bare one (LLP 1104 D6). `outline="none"` (or `outline-style="none"`)
takes it off on every host, for a field whose focus you draw yourself, such as
a ring on the card around a composer: the card's `focusin` and `focusout`
handlers hear the field's focus on every host without making the card a Tab
stop. `focusout` also fires when focus moves between descendants and when the
focused node is removed; use its `FocusEvent.relatedTarget` to identify a
subtree exit. `stopPropagation()` stops a focus event at its current handler;
autofocus delivers these events too, with IDs read when focus moves.
A field with no visible focus fails WCAG 2.4.7. Tab still reaches the field and its accessibility is unchanged. Only
`none` is admitted, and only on a text field or a `textarea`.

`textarea rows=3` sets its preferred height in lines (default 2); explicit CSS
height and `field-sizing="content"` override it. `maxlength=80` on text inputs
and textareas limits user edits in UTF-16 units; authored `value` updates are
not truncated. It does not apply to `input type="number"`.

`intersect=seen` observes geometric intersection with the nearest ancestor
scroll container (or viewport). `action seen(e: IntersectionObserverEntry)`
reads `e.isIntersecting` and `e.intersectionRatio`; authored arguments precede
the optional record. It fires once on mount, including offscreen, then on
enter/leave at threshold 0. The ratio is a snapshot, not a continuous progress
signal; edge contact may be true with ratio 0. `intersect-margin="200px"`
expands the root for overscan; a literal of one to four `px`/`%` lengths (units
are case-insensitive) has IntersectionObserver's `rootMargin` semantics (percentages use the corresponding
root dimension, as in Chrome).
A native route's scrollport also counts as a root. `overflow: clip` remains
an intervening clip, and margins expand only the observer root.
Removing a node stops observation, with no synthetic leave. Both web targets
use the browser. AppKit, UIKit and Linux sample native layout and scrolling;
inline runs, moving presentation geometry, rotated roots and SVG clip references
remain unqualified (LLP 1005, intersection visibility; LLP 1001 records the
percentage-margin spec deviation). Other native hosts remain unqualified.
Container queries and live-position feedback are still deferred.

`resize="none"` disables browser resize handles. Other CSS resize values are
refused with a native geometry explanation. Given an action instead,
`resize=fit` is the element resize event, `ResizeObserver`'s: `fit` hears the
content box's width and height after the first layout and whenever they change
(one more parameter: its `DOMRectReadOnly`), on every host
([Events](contract-grammar.md#events)); read other boxes there with `frame(id)`
rather than polling with a timer. `user-select="none"` prevents
ordinary text selection; `auto` is the default, and on the Mac it is AppKit's:
a label inside a button, link or other control, or in a `header`, `nav`,
`footer`, toolbar, tablist or menu, is not selectable, while an `article`'s text
and other content text is (so put prose a reader copies in `article`, not a
`header`). On iOS `auto` is UIKit's, where a label selects nothing: write
`user-select="text"` on prose a reader copies (on the text or anything above
it), and a long press or a double tap there selects a word with the system's
handles and its Copy, Look Up, Translate and Share menu, one paragraph (one
`text`) at a time. A label inside a control stays unselectable unless the
`text` is said on it or inside the control. On the web `auto` leaves the
choice to the page's stylesheet, but a control's label (a button, link,
pressable or control role) is not selectable there either; a text's own runs
select with it. `user-select="all"` makes an element's text one unit, as CSS
UI 4 has it (a path, a device code): on the web and macOS one click in it
selects all of it, and a drag, a Shift-click
or Shift with an arrow that reaches into it takes it whole. On iOS a long
press or a double tap in it selects it instead of a word, and a tap selects
nothing, as on a `UITextView`; an element around a paragraph selects that
paragraph (one selection is one paragraph there). An inline run that says
`all` is selectable in text that is not (a label, a `header`), alone, on
all three. Text that presses (`press=`) still presses on each click or
tap; on macOS it does not select, and on iOS a tap while it holds a
selection clears the selection, as in a text view. Linux, Windows, Android
and tvOS have no text
selection, so `text` and `all` change nothing there. `contain` is refused by
name: no native host has it. These rows take literals
or choices of literals, so unsupported runtime values cannot bypass the check.

**Colours: say a role, not a value.** Leave a colour unsaid where you can (text,
tint, page and sheet backgrounds, separators and controls are the platform's).
Where you must say one, name a role, which each host resolves to its own colour
for light, dark and Increased Contrast (`labelColor` on iOS, the browser's own on
the web; LLP 1095): CSS's system colours `Canvas`, `CanvasText`, `LinkText`,
`GrayText`, `AccentColor`, `AccentColorText`, `Field`, `FieldText`, `ButtonFace`,
`ButtonText`, `Highlight`, `HighlightText` (`AccentColor` and `LinkText` are the
tint: the nearest `accent-color` a node or an ancestor sets, else the platform's, as
UIKit's inherited `tintColor` is; `AccentColorText` is the ink on that accent, on the web
black or white by its lightness as a filled button's label), and Exact's `-exact-label`,
`-exact-secondary-label`, `-exact-tertiary-label`, `-exact-quaternary-label`,
`-exact-placeholder`, `-exact-separator`, `-exact-opaque-separator`, `-exact-link`,
`-exact-symbol` (what a symbol nobody tinted is, the initial `-exact-tint-color`:
the tint on iOS and the web, the secondary label colour on the Mac),
`-exact-background`, `-exact-secondary-background`, `-exact-tertiary-background`,
`-exact-grouped-background`, `-exact-secondary-grouped-background`,
`-exact-tertiary-grouped-background`, `-exact-fill` (and `secondary-`, `tertiary-`,
`quaternary-`), and the hues `-exact-system-red`, `-orange`, `-yellow`, `-green`,
`-mint`, `-teal`, `-cyan`, `-blue`, `-indigo`, `-purple`, `-pink`, `-brown`,
`-gray` (each `-exact-system-…`). `color="-exact-secondary-label"`, not
`color="#8e8e93"`; `border-color="-exact-separator"`, not a grey hex. Surfaces come
in pairs: a card on a plain page (`-exact-background`, or unsaid) is
`-exact-secondary-background`; on a `-exact-grouped-background` page it is
`-exact-secondary-grouped-background` (white in light mode, so on a plain white page
it vanishes); a surface inside either card is its pair's tertiary role. A route whose
scroll is a grouped list (`list appearance="auto"`, `inset-grouped` or `grouped`) is a
`-exact-grouped-background` page with nothing written, bar to bottom edge, so a switcher
or summary between its `header` and the list sits on the list's grey (LLP 1116.007 D5); a
`background-color` on the route, or on that content, wins. One on the list itself is its
ground around the cards, which keep theirs: `transparent` shows what lies behind it, such
as an `aria-hidden` `svg` at `z-index=-1` (LLP 1084 §6.5). A brand colour
the platform has no role for is the one place for a literal.

**Type: say a heading, not a size.** Body text is the platform's body size
already (the root font size: Dynamic Type's body on iOS, 13 on the Mac, the
browser's 16). A `text role="heading"` with no `font-size` or `font-weight` of
its own is the platform's text style for its `aria-level` (LLP 1115 D3): 1
`title1`, 2 (the default) `title2`, 3 `title3`, 4 and on `headline`, sized for
the reader's Dynamic Type by Apple's own ramp (iOS at the default size:
28/22/20/17 pt; the Mac: 22/17/15/13; the web reads the ramp at its
16 px root: 27/21/19/16, in `rem`). iOS, macOS and Material keep each style's
weight and leading. The web, neutral and terminal looks use bold for levels 1
and 2, semibold for 3. Omit heading sizes and weights unless customizing them.
An authored `font`, `font-size` or `font-weight` wins; an inherited one does not.

Native navigation bars and grouped-list headers own title typography and spacing.
On iOS, a route's level-1 heading gives a large navigation title; other levels
give an inline title. Set UIKit's `largeTitleDisplayMode` through a native access
hatch to change presentation while keeping the semantic rank. Check the host's
implementation for its supported APIs.

Other text takes a style by name with `font`, as WebKit's
`font: -apple-system-headline`: `font="-exact-footnote"` (or
`"-apple-system-footnote"`) sets the style's size and a `normal` font style, and
the style's weight and leading as each platform draws it (LLP 1115.000 D12.2:
the Mac's `headline` bold and its styles' own leading, 13/16 for body; iOS's the
face's own), beating an inherited weight or line height as CSS's shorthand
resets them. The styles are `-exact-large-title`, `-exact-title1`, `-exact-title2`,
`-exact-title3`, `-exact-headline`, `-exact-body`, `-exact-callout`,
`-exact-subheadline`, `-exact-footnote`, `-exact-caption1`, `-exact-caption2`;
`font-size="-exact-caption1"` alone sets the size and keeps an inherited weight
and line height, the style filling only what nothing above says.
`font-size="min(-exact-subheadline, 31px)"` grows with the text size up to its
cap, as UIKit's `UIFontMetrics.scaledFont(for:maximumPointSize:)` holds one: for a
row of equal cells that must stay one line at the accessibility sizes (a week's
day numbers, which UIKit's calendar caps at 31 points), never to shrink text. A weight or
line height written on the node wins either way, and `layout` names each one's
source (`style`, `heading`, `inherited`, `authored`). `font` takes nothing else.
Bold Text (iOS's setting, Android's font weight adjustment) weights the system
font one step heavier, a written weight included, as a hand-built app's text
is, and leaves a family the app names alone: write the weight you mean, not one
you expect the setting to add (LLP 1115.000 D12.3). A drive turns it on, on
Apple, with `prefer bold-text true`.

A colour is any CSS colour the browser paints: hex, `rgb()`, `hsl()`, `hwb()`,
a named colour, `transparent`, `lab()`/`oklch()`/`color()` (clipped to sRGB
natively), or `light-dark(a, b)`; the kernel parses it once for every host.
An app names its own colours once, in a root `theme` block, one
`name="colour"` line an entry: a colour, a role (`card="secondary-grouped-background"`)
or another entry's name. A row writes the name as any colour, in every colour
row and inside shadows and gradients (`color="brand"`,
`box-shadow="0 1px 2px brand"`), and a string from data naming it reads the same
colour. An entry named like a role (`-exact-secondary-label`, or LLP 1095 D4's
bare `secondary-label`, or `Canvas`) overrides that role where a row names it.
Cycles, and a name inside `light-dark()`, are refused (LLP 1095 D4).
An app with a brand, or modes the person picks, writes its design in
`app.tamber` beside `app.contract` (or the file `app.json`'s `"tamber"` names)
instead (LLP 1121): a `theme` block of tokens (`surface.card =
light-dark(#f1f3f5, #16181c)`, written as CSS, dotted names allowed), and an
`axes` block of app modes (`dark-style = black | dim  under scheme=dark`), each
token's value in a mode on its own `when dark-style=dim: …` line. Rows name a
token as they name a role, and an expression over tokens
(`color-mix(in oklab, accent.fill 40%, surface.card)`) is folded by the build,
written whole, in a choice's arm or inside a shadow; a mix of colours alone
(`color-mix(in srgb, red 35%, transparent)`) is a token in the `theme` block.
`setMode("dark-style", "dim")` sets the window's mode; `-exact-mode="dark-style=black"`
on a node holds its subtree in one (pins are `axis=value`, by spaces or commas;
`axis:value` is refused), and inherited text inside takes the pin's value. The
agent's `prefer mode dark-style=dim` sets it from a drive (the app's own
`setMode` still wins after it: an app that restores a kept mode at launch
overrides an earlier `prefer`, so prefer after `clock data`; a dev reload keeps
the window's mode unless the edit removed it), `layout <id>` shows
the token beside each row's value, and `bun exact.mjs tamber check | eval --mode
… | ir | fmt` checks the design, prints its tokens in a mode, writes its IR, or
puts it in its one layout (`fmt --check` changes nothing). The
rule: a role if the platform has one, a token if it is the brand. A token one
colour in every scheme by design (white ink on a brand fill, a black ground
under media) says so with `scheme=fixed` after its value (`white_100 = #ffffff
scheme=fixed`), and `lower-colour-one-scheme` leaves it alone; an unmarked one
is reported once, at its line in the design. `pair=surface.page` after a
foreground's value names what it is drawn on (a token or a role);
`contract tamber check` warns when the two fall below 4.5:1 (`tamber-contrast`).
The brand's states come from the theme, not a hatch (LLP 1129): `AccentColor =
accent` is the app's tint, so the keyboard's focus ring is drawn in it (on the web
a solid 2 px line, which reads on a control filled with the accent); `Highlight`
and `HighlightText` colour selected text on the web (iOS draws it in the tint); a
`button` you fill (`background-color="accent"`) hovers in its own ink, as the
filled button does, and a row or card of your own (a `link` or a pressable)
hovers in the brand's colour by a `:hover` part; a sheet's corners are
`border-radius` on its route. A button
in the brand's shape is a face you write on a `button` with no `buttonStyle`; a
`buttonStyle` is the platform's control, which takes the accent and no radius
(`lower-button-style`), and `ButtonText` and `LinkText` are the tint's, not the
theme's (`lower-theme-tint`).
A type token is CSS `font` syntax (`type.body = -exact-body/1.35 "InterVariable"`):
`font="type.body"` writes only the longhands it names, so the text style's own
weight applies where it names none, and a size in px is warned of
(`tamber-type-fixed`: it does not follow the person's text size). A family token
is a stack (`family.text = "InterVariable", system-ui`), named by `font-family`.
One that varies by a mode goes on the text, not its container
(`tamber-inherited-axis`); on the JS target each row it writes is a property
per mode (`--x-type-body-size`).
A component declares what a designer may tweak in a `params` section (LLP 1121
D15), one param a line: `radius: length = "radius.md"  range="0..40"  step=1
control="slider"  group="Shape"  priority=1  label="Corner radius"`. A param is a
token the component owns: its default is any token value (a literal, a token, or
an expression over tokens and the component's other params), and its rows and
defaults name it `param.radius`. Every instance shares it. Its full name is the
component's declaring file and name, `post.contract#PostButton.radius` (an import
alias never changes it; moving the file makes it a new param), and
`PostButton.radius` where no other component has that name (a package's file
is its package's however a `use` reaches it). A param is a row's value in its
component's own rows (a style, shorthand or motion row, an image's `symbol:`
source): a string or a template, through `?:` branches, `match` arms and `+`,
as a motion token is; and its defaults. Passed in a component's argument, it is
the child's row's value. One in a `style` that one component's view alone
applies is that component's (LLP 1139 D3); one in a `style` another
component applies too, a `keyframes` or a state's value,
or in what a row computes (a function's argument, a condition, a comparison,
a callback), or that a child reads otherwise than as a row's value, is refused
(`tamber-param-scope`). A param no
row of its component reads (a state part's row counts: `:active`'s
`background-color = "param.pressed"`), directly or through another param, is refused
(`tamber-param-unused`); a ranged length is px (`tamber-range-unit`), or a base
name or an expression over one, checked in every platform's column, and its
default must be inside the range in every mode; `control` follows the kind
(`color`, `slider`, `stepper`, `token`, `select` with `options="a|b"`, `none`),
and `priority` is 1 to 3. A design token may not be named `param.…`.
`contract tamber ir` and the design server's `describe` list them under
`components`; `contract tamber check` builds an app with params to admit its
design (a dial only a param reads is read), counts them apart, and charges a
refusal or warning on one to its default. A view that isn't Contract (a native module's, a Rust canvas
component) declares its params in the design instead: `component MapCard`, then
an indented `params` block of lines like a dial's, any kind (`inset: length =
8px  in 0..24  control=slider  priority=2`). Its identity is the design file
and the name (`app.tamber#MapCard`); its declaration keeps every param, and
what its default reads, as the code reads them by name; a `when` is not the
component's (the app's design sets a param per mode).
The app sets a param's value in its
design's `params` block (`PostButton.radius = 8px`, `when` lines too), which
every instance shares; the design server's `set-param` writes it,
`unset-param` takes it back to the component's default, and a line naming no
param of the app is warned of.
A package can ship a design (LLP 1121 D11): a file named for the last part of
its `package.json` name (`@exact/reading` → `reading.tamber`) beside its entry
file (what `exports` maps `.` to, else `index.contract`), or the file its
`package.json` `"tamber"` names inside the package. It starts `tamber reading`
and holds one `theme` block of tokens named `reading.…`, which the package's
views name. An app that uses the package gets them. The app's design sets one
by its full name (`reading.card = #abcdef` in `app.tamber`), which replaces the
package's, `when` lines and all. A package's design may declare its
components' params (`component Map` with `params`, named
`<package>/<file>#Map`, as its Contract components are), whose values the
app's design sets (`Map.pin = #000000` under `params`, or the full name). It
may declare icons (`icons`, `reading.star = icon(sf "star", svg "star.svg")`),
each named `reading.…` and naming only its own package's icons; the app's
design overrides one by full name under its `icons`, whole. An icon's SVG path
is relative to the `assets/` of the root whose file writes it: the app's
directory for the app's, the package's root for a package's (an icon's, or a
param default's `icon(svg …)` in its design or its Contract), so a package
whose `"tamber"` is `theme/palette.tamber` still reads `<package>/assets/`; in v1 it must be a one-colour template
(`tamber-icon-svg`). It declares no axes, dials, `params` lines or `exports`
(`tamber-package-block`; its `when` lines name the app's axes), declares only `reading.…` names and none of the
base names (`tamber-package-name`), and a token in it names only tokens the
same file declares, roles, and the base names `space.xs`…`space.xl` and
`radius.sm`…`radius.full`, which the app's design defines until the base scale
ships (`tamber-unknown-token`). An app can't use two packages whose names end
alike (`@acme/reading` and `@other/reading`) when both ship a design. A
refusal points at its line in the package's file. `contract tamber
check|eval|ir|gen` and the design server fold the app's design with its
packages' tokens, as the build does.
A length token may name a base name bare (`card.pad: length = space.md`) or in
a `calc()`, `min()`, `max()` or `clamp()` with px and numbers
(`calc(space.md * 2 + 4px)`): each host reads it in its own column, and the
compiler checks every column, naming the one a row refuses (a padding that is
negative on mac). A shadow token's lengths may be any of these
(`card.shadow: box-shadow = 0 space.md calc(space.sm * 2) black`), checked in
every column the same way (a blur negative on mac is refused); a literal
`box-shadow` row's lengths stay px, so name a shadow token there. A base name
times a length, and `rem`, `em` or `%` beside one, are refused.
A package that is `"private": true` and ships a design is a library (LLP
1139): each component its entry declares or re-exports is held to its rules.
Its spacing, corner and colour rows (a shorthand's each side; `border`'s
colour), and its params' defaults of every kind, name a param, a token, a
role or a base name, or compute over one (`color-mix`, `light-dark`,
`calc`), never a literal (`0`/`0px` and CSS keywords such as `auto` and
`transparent` excepted; `lower-library-literal`); a control's role is the native control's (`input`,
`textarea`, `select`, `progress`, a native `button`) with what its platform
needs (`checked` and `aria-label` or `aria-labelledby`, a range's
`min`/`max`, …;
`lower-library-control`); each of its buttons (a `button`, or a node of
`role="button"`) is named, by `aria-label`, `aria-labelledby` or a text of
its own, and one whose look reads a state prop (a bool named `selected`,
`current`, `pressed`, `checked`, `on`, `active` or `expanded`, in a `?:`,
`match` or class) says it aloud through `aria-selected`, `aria-pressed`,
`aria-current`, `aria-checked` or `aria-expanded`, or a label that reads it
(also `lower-library-control`; any other bool, a size or an ink, is
presentation); and nothing in the package reads the host it runs
on (`exactDelivery`, `exactSurface`, `exactViewport().displayMode` read
from the viewport wherever the package passes it: an alias, a `provide`, a
`fn`; a prop's `displayMode` is the app's data; `lower-library-platform`). Not checked yet: an injected value whose
provider is outside the package, and per-platform host entries (no syntax
for them). Its names are its API: an app line or a Contract row naming a
name under its namespace that it doesn't define, or a param line naming one
it lacks, fails the build (`tamber-package-unknown`; the nine base names are
no package's), and its params are named in full, a short name that names
none too (`tamber-param-short-library`). The in-repo `@exact/actions`'
`BarItem` and `HeaderButton` are for an app's own authored bar and header (a
design's, as Bluesky's are); the platform's `tabs` and `header` routes stay
the platform's, and no library component wraps them. A `BarItem` says its
selection as the bar's current item (`aria-current="page"`, iOS's selected
trait) and takes no long press itself: an item held for more puts
`contextmenu` on its slot fill (Bluesky's Profile avatar), so the press lands
on the fill rather than the whole column.
The in-repo packages' one-colour nonzero icons are icon tokens, which an app
overrides by full name: `@exact/cards`' `cards.repost`, `cards.pin`,
`cards.corner-down-right` (PostCard's context and reply lines), `cards.ban`,
`cards.eye-slash`, `cards.warning`, `cards.circle-info` (ProfileHeader's
alerts), and `@exact/actions`' `actions.reply`, `actions.repost`,
`actions.heart`, `actions.heart-filled`, `actions.bookmark`,
`actions.bookmark-filled`, `actions.share`, `actions.dots` (ActionRow). Each
has no `sf`, so Apple draws the same path as the web. Still paths, as no
template can draw them yet (QUEUE): ComposeFrame's gallery, camera and GIF,
`BarItem`'s and `HeaderButton`'s icons and AtCard's Attie mark fill even-odd
(a template takes `fill-rule="evenodd"` since stage 14 C2; they move to tokens
in a later change), and the verified and verifier badges are two colours (a
template is one).
Code outside a view names a token by its name too (LLP 1121 D12): a data module
returns a `ColorToken` (or a `LengthToken`, `TypeToken`, … of its kind:
`contract types` declares the design's, so an unknown one fails `tsc`), and `bun exact.mjs tamber gen ts|rust|swift|kotlin .` writes bindings into
`.exact/tamber/` (never committed): `tamber.ts` and `tamber.css` (each token's
CSS per mode), `tamber.rs` (`pub const SURFACE_CARD: Token<Color>`, and
`Token<Length>`, `Token<Number>`, `Token<Type>` for those kinds, which
`session.tamber().get(SURFACE_CARD, place)` resolves in a running session), or
`tamber.swift` (`ExactTamberToken.surfaceCard`, for a native view's
`ExactTamber(view).color(.surfaceCard)`, `.length(_:)`, `.number(_:)` and
`.font(_:fallback:)`, a font at the person's text size with Bold Text's step),
or `TamberTokens.kt` (`TamberColor.SURFACE_CARD`, `TamberLength`,
`TamberNumber`, for `ExactTamber(view).color(…)` in the app's hatches). A
component's param is named by `<Component>.<param>` there (`mapCardInset`,
`MAP_CARD_INSET`); two names that map alike are refused, naming both. The Apple
build writes `tamber.swift` and the Android reader's `TamberTokens.kt` before
an app's modules compile.
Recipe:
[a brand](recipes/tamber.md).
`color-scheme="dark"` (or `"light"`) on a node makes that subtree resolve
`light-dark()`, platform colours and glass in that scheme, as a sheet that is
always dark does; leave it off to follow the surrounding scheme (LLP 1034 §8).
`status-bar-style="light-content"` (light text, for a dark surface),
`"dark-content"` or `"auto"` on any node, bound to state, sets an iOS phone's
status bar: of what the bar sits over, the declaration painted on top wins, and
a flip shows in its own batch's frame; `status-bar-animation="fade"` fades it
(LLP 1105). Other hosts ignore both.
`wakelock="screen"` on any node, bound to state (`wakelock=(playing ? "screen" :
"none")`), keeps the display awake while that node is displayed; a hidden or
unmounted one holds nothing (LLP 1125 D11; `state.wakelock` is `{held, by}`).
`currentcolor` takes the node's `color` on borders, `background-color`,
`-exact-tint-color`, text stroke and SVG paint. `unset` clears any row, and `inherit`
an inherited one (`color`, fonts, `fill`…); `inherit` on a row CSS does not
inherit is refused. `order` places flex and grid items. An image's accessible
name is `alt` or `aria-label`; `enterkeyhint` labels a soft keyboard's enter
key on the web and iOS. A bare number on a length row is pixels (except
`line-height`, where it is CSS's multiple of the font size), and the row takes
CSS's spellings too (`font-size="14px"`, `letter-spacing="-0.5px"`,
`padding="1.5rem"`; `stroke-width="2px"` but no `rem` there), bound or literal
(``font-size=`${size}px` ``). A separator's thickness is `-exact-hairline`, one
device pixel on whatever display the app is on (a third of a point on a phone,
half on a Retina Mac, a pixel on a 1x screen), where `0.5` is one only at 2x:
`border-bottom="-exact-hairline solid -exact-separator"`, or `hr`, which is that
line with no margin of its own (space around it is the `gap` or `margin` you
write). Between the rows of a `list` write neither: its children are rows, and
the separator there is the platform's (LLP 1115.000 D16). A row whose value is a number and no text
(`opacity`, `flex-grow`, `z-index`, `font-weight`, `column-count`,
`column-rule-width`) takes a number where it is computed: a string-typed
expression there is refused, since the native hosts read no text on it (the
literal keywords and `px` of `column-count` and `column-rule-width` compile to
their numbers). `max-width` and `max-height` take `none`, CSS's initial maximum, or `auto`, and the web writes
`none` for either. A number field's (`input type="number"`, written so)
`min`, `max` and `step` take numbers, as a range's do; its `value` is its text.
On the Mac, Up/Down and accessibility increment/decrement step the number even with
`appearance="textfield"`. Apple number fields filter typing and paste while keeping signs,
fractions, exponents and input-method composition; `inputmode` alone does not filter text.

`autocomplete` on an `input` or `textarea` is HTML's attribute, written as
HTML writes it (`autocomplete="username"`, `"section-login current-password"`,
`"shipping postal-code"`). The web sets it as written. iOS and macOS read its
last field name (a trailing `webauthn` aside) as the field's AutoFill content
type, over the one `type` implies (iOS: `password`, `email`): `username`,
`current-password`, `new-password`, `one-time-code`, `email`, `tel`, `url`,
`name`, `given-name`, `additional-name`, `family-name`, `honorific-prefix`,
`honorific-suffix`, `nickname`, `organization`, `organization-title`,
`street-address`, `address-line1`, `address-line2`, `address-level1`…`3`,
`postal-code`, `country-name`, `cc-name`, `cc-given-name`,
`cc-additional-name`, `cc-family-name`, `cc-number`, `cc-exp`, `cc-exp-month`,
`cc-exp-year`, `cc-csc`, `cc-type`, `bday`, `bday-day`, `bday-month` and
`bday-year`.
macOS 26 offers verification codes from Messages and Mail in every text field;
`app.json`'s `host.macos.oneTimeCodeAutofill: "marked"` keeps them to fields
marked `one-time-code`.
`off` clears the content type (the web's `autocomplete="off"`); `on` or a
list HTML's grammar refuses (the web's default) leaves `type`'s, and so does
a name the platform has no type for (`country`, `impp`, `sex`), which only the
web can act on.

An `input type="password"`'s value is never agent output, on any host: `tree`,
`layout <field>` and the `type` reply show `value="•••"` for any value that is
not empty, whatever its length (an `expect text` on the field reads `•••`), and
`tree --ax` marks the field `protected` where the host has one (the web's
accessibility tree still shows one bullet a character, as Chrome exposes it).
What the app stores (`state.slots`, its data module) is the app's own; an
`app.test.contract` `type … append` into a filled password field is refused.

`aria-busy=(not ready)` marks a region as still loading: assistive technology
hears it, and time to interactive (`modules/observe`) waits until no element is
busy.

`border`, `border-top/right/bottom/left` take CSS width/style/color in any order,
resetting omitted components to medium/none/currentcolor. Widths are px/pt,
unitless zero, or thin/medium/thick (1/3/5 px); styles are none/hidden/solid/inset/dotted/dashed.
The web and native GUI hosts paint dotted and dashed while keeping their widths.
At 1×, AppKit and Linux dot and curved-join pixels have a bounded raster
difference from Chrome. UIKit uses AppKit's painter; its simulator parity is
still unmeasured (LLP 1001, border semantics).
Other CSS border styles are diagnosed as native painter gaps. `text-decoration`
takes none, underline, line-through, or both lines; solid/currentcolor/auto
components retain the supported defaults. Color/style/thickness extensions are
refused by name. Linux paints solid lines with UA metrics and diagnoses its
missing underline skip-ink behavior in the driver log.

A flex shorthand's grow/shrink factors must be adjacent. Intrinsic flex-basis
keywords (content/min-content/max-content/fit-content) are CSS values, but the
current dimension representation cannot size that mode; the diagnostic names
this limit. `auto` takes the basis from the authored main-size property.

Font lists distinguish quoted local names from unquoted CSS generics. The eight
mapped generics are system-ui, ui-sans-serif, sans-serif, ui-serif, serif,
ui-monospace, monospace and ui-rounded. Other CSS generics and CSS-wide
font-family values are refused with the native mapping/stack representation
reason, rather than being silently treated as local family names.
