# Contract: a complete working guide for agents

Use this guide to author, change, inspect, and verify a current Exact application.
It covers the language implemented on `main` on 2026-10-02. Start here for an
implementation task; use the [human guide](contract-for-humans.md) for explanations
and complete examples, and the [grammar reference](contract-grammar.md) for exact
forms, built-in functions, tags, and event payloads.

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
| Derived labels, record copies, bounded list transforms | Contract expressions / `fn` |
| Network, authentication, storage, sorting, domain algorithms | App TypeScript/Rust data module |
| Device facts | Reserved source with an admitted shape |
| A system control (button, list, switch, picker, menu, tabs, bars) | Contract's native form ([below](#views-layout-and-interaction)); an app native module only where none exists |
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
`app.json`'s `commands` (`"verify": ["bun", "verify.mjs"]` is `bun exact.mjs verify
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
independent diagnostics it can collect, capped at 20; success is `[]`. Keep the
exit status: 0 success, 1 compilation/I/O failure, 2 invalid invocation. Read all
diagnostics, including related locations, before making the next repair.

Formatting is explicit. `fmt --stdout` previews, `fmt --check` checks, and plain
`fmt` writes. Formatting changes spacing and breaks only: a result that would
parse to a different program is refused (`fmt-tree-change`) and nothing is
written. Avoid formatting unrelated files. `symbols` reports definitions
and references, with component interfaces and inferred action effects. Search
by exact name with `symbols file.contract --name name`.

After compiling, build and drive the actual app. From an `exact new` app:

```sh
bun exact.mjs test web                          # app.test.contract; builds a stale web app first
bun exact.mjs agent web tree "tap add" state logs "screenshot out.png"
bun exact.mjs agent web --size 390x844 "screenshot phone.png"   # a phone-sized viewport
```

The web carrier opens at 420×900; `--size <w>x<h>` (before the operations) opens
another viewport, and a test's first step `size <w>x<h>` does the same for that test.
`resize <w>x<h>`, an operation and a test step, resizes it mid-drive as a person
dragging the window's edge would: the browser's viewport, a macOS window, the
Linux presenter. iOS refuses a mid-drive `resize`; a size given at launch (`--size`, a
test's `size` line) is laid out at that viewport with no safe-area insets, scaled to fit
the app's frame (the safe area, unless the root covers the whole screen).
`close`, an operation and a test step, presses the window's close button as ⌘W
or the red button would (the agent's window is never key, so a typed ⌘W reaches
nothing): the window asks its `beforeunload` first, and the reply says
`closed: true`, or `closed: false` for a window a handler kept (the web's
"Leave site?" is answered "Stay"). macOS and the web; iOS and Linux refuse it.
`--storage <name>` gives a drive a named scratch store, kept between drives on
macOS, iOS, Linux and web Chrome (a kept browser profile served on one port per
name; a Firefox or WebKit drive starts fresh each time, so a name there lasts
for the drive, including its `reload`). Without it a drive has no
app storage: an `app:/` file or SQLite request is refused (`storage is unavailable
in agent mode unless the drive names a scratch store (--storage <name>)`), so an
app that keeps its data there does nothing on a write; a chosen document (`doc:/`)
still opens under its grant. That store, and an authored test's own store, includes
a data module's `secret.keep`: files in the named scratch tree on Apple and Linux,
and the page's `localStorage` on the web. A nameless drive keeps no secrets.
To show what survives a restart on any host, use an authored test's `reload` step (below).

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
| `use Card from "@acme/ui"` / `use Activity from "exact:motion"` | File | A package's names (from `node_modules`), or a built-in's |
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
| `state name = expr` | Component | Stored state |
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
state, prop, parameter, `let`, `each` item, shape name, …): `when`, `if`, `else`,
`each`, `in`, `match`, `case`, `as`, `fn`, `and`, `or`, `not`, `true`, `false`,
`none`, `some`. Every other keyword (`key`, `state`, `from`, `refresh`, `view`, …)
is an ordinary name there, so `action searchKey(key: string)` and `action refresh`
compile. Shape fields, named arguments and members take any word
([grammar](contract-grammar.md#lexical-rules)).

`contract vocab` lists every built-in tag, attribute, and CSS property the compiler
accepts, with each property's value kind and default; `contract vocab <name>`
answers for one, or suggests the spelling it meant. Check there before guessing
at CSS.

## Values, expressions, and functions

- Types are `number`, `string`, `bool`, declared shapes, `option<T>`, `list<T>`,
  and `action` for behavior interfaces. Do not emit authored `any` or object types.
- `[a, b, c]` is a list of its items, which may span lines and keep a trailing
  comma; the items have one type, met as a ternary's arms are (`[none,
  some(1)]` is a `list<option<number>>`; `[1, "a"]` is `type-list-item`).
  `concat(xs, ys)`, `slice(xs, start, end?)`, `includes(xs, x)` and
  `indexOf(xs, x)` are the web's array methods: `[...xs, x]` is
  `concat(xs, [x])`, `includes` finds a string, number or bool (by
  SameValueZero, as the web does) and `indexOf` says where (by `===`: `NaN` is
  never found, `-1` for none). Over text, `indexOf(s, t)` is a position in
  UTF-16 code units, and `split(s, ", ")` cuts text into a `list<string>`
  (recipient chips). A list the
  screen keeps for the session — a selection, open or collapsed ids, per-row
  offsets — is `state` built this way; a list the app keeps across launches, or
  a server owns, belongs to the data module, and so do sorting, grouping and
  aggregates:

  ```contract
  state collapsed = []
  action toggle(id: string)
    collapsed = includes(collapsed, id) ? filter(collapsed, c => c != id) : concat(collapsed, [id])
  ```
- `none` and `[]` need an inferable element type. A state initialized by either
  usually gets that information from later assignments; a typed argument or
  the other conditional/match arm can also supply it.
- An option directly inside an option is refused (`type-option-option`):
  `option<option<T>>` written, `some(none)`, `first` or `at` of a
  `list<option<T>>`, a mutation `as shape option<T>`. The web erases `some`,
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
  per-row state in a component used inside `each`.
- `Shape(field=value, …)` constructs every field exactly once.
  `Shape(base, field=value, …)` copies a base of the same shape and replaces fields.
  The one positional base comes first. Compiler-owned shapes are not constructible.
- Use field access, arithmetic, comparisons, boolean operators, ternaries, and
  exhaustive option matches. There is no truthiness or optional chaining.
- Inline match is `match x { case some(v) => a, case none => b }`; the comma is
  required and the `some` arm comes first. The block statement/view form has
  separate indented `case` arms.
- `map(xs, (x, i) => expr)` and `filter(xs, x => bool)` return values, not nodes.
  `join(xs, separator)` accepts primitive items. `first(xs)` and `at(xs, i)`
  return options; `at` supports negative indices.
- Two strings compare with `<`, `<=`, `>`, `>=` in UTF-16 code-unit order, as on
  the web (`end > start` for `"HH:MM"` times). `slice(s, 0, -1)`,
  `replaceAll(s, find, with)` and `toLowerCase(s)` are the web's string methods.
- Numbers have `floor`, `ceil`, `round` (JavaScript's `Math.round`: `round(-2.5)` is
  -2), `min`, `max`, `%`, `formatNumber` (`1.2K`), `toFixed` and `formatDecimal`. A
  field's text is a number through `match parseNumber(s) { case some(n) => …, case
  none => … }`: a decimal numeral, trimmed, or `none` (`"12px"`, `""`).
- Money is `formatDecimal(round(price * 100), 2)`: an integer count of cents
  printed exactly (`1234` is `"12.34"`, `-5` is `"-0.05"`), so `19.995` shows
  `20.00`. Keep money as a count of cents where you can; `formatDecimal` of a
  number that is not an integer prints `""`, so round first. `toFixed(x, 2)` is
  the web's `x.toFixed(2)`, for a measured number (`toFixed(km, 1)`): it rounds
  the binary value, so `toFixed(1.005, 2)` is `"1.00"`, which is why money does
  not use it. Both take the digits as a whole-number literal (`toFixed` 0–100,
  `formatDecimal` 0–20), never a variable, and print `""` for `NaN` or
  `Infinity` (JavaScript's `toFixed` prints `"NaN"`).

```contract
component Cart
  state price = 19.995
  state km = 12.34
  view
    column
      text `Total $${formatDecimal(round(price * 100), 2)}` testId="total"
      text `${toFixed(km, 1)} km` testId="distance"
```
- Dates: `formatDate(ms, offset, "iso")` is `YYYY-MM-DD`, and `calendarDiff(from, to,
  "years")` (or `"months"`) is the whole periods between two such dates as an
  `option<number>`, counted as an age is (a Feb 29 birthday has its year on Mar 1).
- There is no general list append: add an item to
  resource-backed data in its source and answer the updated list (a mutation that
  `refreshes` the list's resource, or its own answer).
- Standard calls are free functions, not methods: `trim(s)`, `includes(s, q)`.
  There are no object literals, spreads, general lambdas, array indexing,
  assignment expressions, or JavaScript built-ins by implication.
- `fn label(done: bool): string = done ? "Done" : "Open"` is a function: parameters
  and the return type are explicit, after `:` (not `->`). Its body is one expression over
  its parameters and standard calls (including `now()`), without component-state
  capture or recursion. Pass an app value in; do not invent an ambient reference.
  A `fn` named like a standard function (`fn indexOf`) shadows it in every
  expression of the app, so a standard function added later never breaks an
  app that had the name first; an action or a host command keeps its own rule.
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
and to locals declared after a state assignment.

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
action by name, as a statement, anywhere a statement goes (LLP 1089). The call is
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

## Composition and lifetime

The first component is the root. Each component use is `Name(prop=value, …)`.
Supply every declared prop exactly once; props have no defaults (pass `none` for
an option). An `action` prop can receive a reference with
captured arguments; the eventual event payload is appended at invocation.

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
actions. A child used under `each` owns row state keyed to that row's identity.
Stable keys matter when content is inserted, removed, filtered, or reordered.

A state cannot start from a resource, but a child's state can start from a prop.
So a form that edits a saved record is a child made once the record is in: the
source answers `loaded: true`, the placeholder says `false`, and the child's
states take the record's fields when it is made. The initializer runs once per
child: a later `draft` (a refresh, a normalized answer) does not reset the form,
and while a refresh is out `saved` keeps its value, so the editor stays.

A native host (and the wasm web target; the default JS target keeps none) can make
that child from a kept answer. When the data module is not ready at boot, a resource
whose source read device state (a file, a database, a secret, a watched topic) shows
the last answer the runner kept for it (a small answer, asked with the same
arguments), so the form's states take those fields. A write the resource does not hear about (a file or database save from
a mutation without `refreshes saved`, not a `store` write it reads) leaves that
answer behind: after a restart the form opens with the old fields and keeps them
when the fresh answer lands. Refresh the resource after each write, as below, or key
the child by a string or number from the answer (`each d in [saved] key=…`) so a
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
      when saved.loaded
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

## Data requests and side effects

Choose the mechanism from its lifetime:

| Requirement | Mechanism |
| --- | --- |
| Value follows query arguments | `resource result = source(args) as shape T` |
| Source needs changing context while the same kept answer is suitable at boot | `resource result = source(identity) with context as shape T` |
| Explicit command with a reply | `mutation reply as shape T`, then `send reply = source(args)` |
| Re-request current resource arguments | `refresh result` |
| Refresh reads around a mutation | `mutation … refreshes resourceA, resourceB` |
| React once to a settled mutation | `mutation … then actionName` |
| Writes that must all land, in order | `mutation … queue`: one in flight, later sends wait their turn |
| A timer while something shows | `task … when cond`, restarted by `key=` |
| Pending indicator | `pending(resourceOrMutationName)` |
| Resource request failed without an answer | `failed(resourceName)` (a resource only: a mutation answers its failure as a domain result, such as `ok: false`) |
| Initial resource fallback | `else empty(field=constant)`, or `else source(values)` answered once at build |

Resources read as their declared type. Mutations read as `option<T>` and start at
`none`, so a mutation's `T` is not itself an option. Do not treat a resource as an optional wrapper unless its declared type
itself is optional. A mutation reply is unwrapped with match.

`with` takes one or more expressions, before `as shape`, and appends them to the
source's arguments. All arguments still trigger re-asks and identify live
requests. Only an eligible persisted answer admitted while the source is unready
at boot matches the call arguments alone; it can then stand while activation is
pending. Without `with`, every argument identifies that kept answer. Keep
account/tenant IDs and representation choices in the call, credentials below
the seam, and use `refresh`/`refreshes` for counters whose only purpose is another
ask. The default web JS target keeps no persisted resource answers
([LLP 1027.005](../llp/1027.005-resource-identity-and-request-context.rfc.md)).

The current request owns its answer; older replies cannot overwrite a newer
request. A newer send or an assignment forgets a mutation's in-flight reply, not
its work: the source's storage steps for it still run to their end on every host,
as a browser runs a promise nobody awaits. Because the reply is forgotten, an
action that sends one mutation twice on one path is refused (`analyze-send-twice`), counting
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
65th refuses its action. `refreshes` re-reads
its resources when the mutation is sent (an answer the source gives at once shows
immediately) and forces them again when the reply lands; a mutation the source
answers at once has landed, so its resources are forced in the sending commit (an
async read is asked again, not dropped). `then` is parameterless,
runs once at the host's next clock advance as a new commit (under the driver, an
input's own answer's `then` before the input's reply), reads the latest
answer, does not run for a failure that brought no answer, and cannot send its
own mutation. Do not mistake the scheduling boundary
for a general async workflow or a per-reply event log.

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
The [human guide's data-module section](contract-for-humans.md#writing-the-data-module)
has a complete `app.ts`: synchronous, `fetch` and SQLite sources, the grants
each needs (one per line: `['sqlite.open app:/data/books.db', 'net.fetch https://…'].join('\n')`;
`net.fetch` takes an `http` or `https` origin, `http://127.0.0.1:8080` too; on iOS
cleartext `http` reaches only a local host, and only with `app.json`'s
`host.ios.localNetworking` set),
and how to drive it with storage.
The compiler accepting a source call does not provide its implementation. Check
its arguments, declared result, grants, storage access, and bake-time behavior.
Keep generated output out of version control. Use app-local sources for domain
formatting or algorithms beyond the finite standard roster.

A bake runs initial data work and packages first-frame values. A first-frame
value is not the answer: every host asks the TypeScript module again at launch,
natively once it loads after first pixel, even a source with no arguments
(`logs`: `<resource> shows its build-time answer until its source answers`, then
`<resource> answered: …`). A `send` an action makes before the module loads (a
document opened at launch) waits for it, `pending` meanwhile (`logs`: `send <name>:
waits until the data source is ready`). Live requests run after that under host scheduling. Do not assume a secret store, disk database,
or authenticated network session is available while baking. See
[the data-module reference](reference.md#generate-typescript-data-source-types).

## Views, layout, and interaction

The view forms are element, component use, `when`/`else` (and `else when`),
keyed `each`, exhaustive option `match`, and `children`. Wrap root regions in a
stable element.
HTML names Contract spells otherwise (the compiler names each): `div` is `column`,
`row` or `view`; `span`, `p`, `strong`, `em`, `b` and `i` are `text`; `h1`–`h6` are
`text role="heading" aria-level=N`; `label` is `text` beside its field, which
`aria-label` (or `aria-labelledby`) names; `img` is `image`; `a` is `link`; `ul`,
`ol` and `li` are a `list` or a `column` of rows; `title` and `meta` are `head
title=… description=…`. A component
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

`class=Style` applies literal style values; node attributes win. Conditional
classes select two named styles with `class=(condition ? A : B)`. Computed values
go on nodes. There is no selector cascade, dynamic style object, or arbitrary
class-string composition. A style branch can mix a number and a CSS keyword in
a property's admitted value space; this does not add general union types.

For scrolling, provide a bound and inspect measured layout. For virtualized
lists, use `list virtualized=true` (no other tag takes it), one direct keyed
`each`, and one flow root per row. A vertical list needs `height`, `max-height`
or a growing `flex`, which may be computed (`max-height=(narrow ? "320px" : "640px")`).
A percentage bounds it only against a definite containing-block height (a plain
`column` is content-sized), and the bake refuses (`bake-scroll-unbounded`) only a
list with no bound at all, in its first 390×844 frame, so look at the list in each
layout it takes. It takes `estimated-item-height`. A horizontal one needs a
literal `display="flex"` and a literal positive `height`, takes
`estimated-item-width`, and refuses wrapping, reversed or right-to-left flow, a
nonzero `gap`, main-axis padding, `justify-content` other than `flex-start`, and
`reorderdrop`. `reorderdrop` belongs only on a vertical `list virtualized=true`
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

A native button is an explicit `button appearance="auto"` after class merging;
an ordinary button remains an authored `appearance="none"` pressable. The switch
must be literal. Native title/symbol children are face data, not general layout.
A symbol-only face needs a nonempty accessible label. `buttonStyle` is a declared
styleable host-policy prop; its names and allowable branches are checked against
`schema.json`'s `buttonStyles`. Follow the native-button allowlist and context
checks in [`controls.rs`](../contract/lower/src/controls.rs), and test the actual
platform look. Do not assume arbitrary custom paint or typography is admitted.

**Prefer native controls.** Write the Contract form and each host draws its own
control; a hand-built lookalike (a painted switch, a row of buttons for tabs, a
drawn title bar) is a bug. On iOS:

| Write | iOS draws |
| --- | --- |
| `button appearance="auto"` (`buttonStyle`) | `UIButton` |
| `list appearance="auto" listStyle="inset-grouped"` of `section`s (`header`, rows, `footer`) | `UICollectionView` list, as Settings ([human guide](contract-for-humans.md#choosing-a-native-button)) |
| `input type="checkbox" switch` | `UISwitch` |
| `input type="range"` | `UISlider` |
| `input type="date"`, `"time"`, `"datetime-local"` | `UIDatePicker` |
| `select` of `option`s | a pop-up button with its menu |
| `popover="auto" role="menu"` of `button`s, opened by `popovertarget` | `UIMenu` (LLP 1021) |
| `role="tablist"`: each tab a symbol over a label / one text or image | `UITabBar` / `UISegmentedControl`, the tablist at least its native height unless `min-height` says otherwise (LLP 1059) |
| a route whose first child is a `header` holding one heading and its buttons | the navigation bar; a level-1 heading (`aria-level=1`) is a large title |
| a route with `navigationPresentation="modal"` | a sheet |

`contract vocab <name>` lists each one's props. A route does not scroll by
itself: its content goes in a `scroll`, `list` or `overflow-y="auto"` box, which
`navigationScroll` names for the bar ("Routes and web documents"). A sheet's swipe down and a pushed screen's edge swipe press the
route's enabled control whose `id` is the root's `navigationBack`; without one
both are refused, as is the swipe on a sheet with `closedby="none"`. On a pushed iOS
route under the platform bar, that control's text becomes the bar's back button title
beside the bar's own chevron (no text shows the chevron alone), so label it `Recipes`,
not `‹ Recipes`.

An `image` source is the same string on every host: a path under the app's
`assets/`, an `http(s)` URL, `symbol:<role>` (the roles are
[`schema.json`](../kernel/tables/schema.json)'s `symbols`; a player's are `play`,
`pause`, their `-fill`s, `skip-back-15`, `skip-forward-15`, `skip-back-30`,
`skip-forward-30`, `speaker`, `speaker-mute` and `moon`), an `app:/data|cache|tmp/…` file
(a picked photo, or one the data module kept with `storage.fs`; it shows after a
relaunch too), or a `data:` URL of at most 1 MiB, past which every host shows
nothing (the web and Apple journal `image refused`). Keep a picked photo by copying it to
`app:/data` and answering that path; never tell hosts apart in the data module
(`HermesInternal`) to choose a source
([LLP 1069.002](../llp/1069.002-media-picker.rfc.md) D7, [LLP 1011](../llp/1011-image-v1.spec.md) §2).

A sound effect is a declared WAV that an action plays ([LLP
1096](../llp/1096-sounds-an-app-can-schedule.rfc.md)): `sound "assets/…wav"` at the
top level (16-bit or float PCM, one or two channels, at most 10 s; the compiler
reads it), then `playSound(src, at=, gain=, group=)` from any action. Every call is
a new voice, so a retrigger is another call. `at=` is the runner's clock (`now()`'s
milliseconds; the past means now), `gain=` a linear 0–1, and a `group=` is
monophonic by start time: a voice ends where the next one in its group starts, as a
drum machine's choke does. `stopSounds()` (or `stopSounds(group=…)`) ends what
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

To keep time (a sequencer, a metronome), schedule ahead on the audio clock rather
than starting each hit when a timer's commit lands: the press schedules the first
window, `[now(), now() + 100)`, and each tick of a coarse timer schedules the next,
`[scheduledTo, now() + 100)`, as a list a `fn` computes (`playSounds(hits)` takes a
list of a shape whose fields are, in order, `src`, `at`, `gain` and `group`). A
timer's commit is at its due time, so a hit planned at `t` lands on the grid:

```text
action start
  playing = true
  playSounds(hitsBetween(song, now(), now() + 100))
  scheduledTo = now() + 100
action tick
  if playing
    playSounds(hitsBetween(song, scheduledTo, now() + 100))
    scheduledTo = now() + 100
action stop
  playing = false
  stopSounds()
```

Tests read the runner's record: `expect sound has "assets/ding.wav" at 0`, with any
of `gain`, `ends` and `by end|group|cut|stop|cancelled`, or `expect sound missing …`;
`state sounds` lists the last 64 voices. `audio` stays HTML's player for long media
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
    bold = includes(` ${s.formats} `, " bold ")
  action embolden
    format("editor", "bold")
  view
    column
      button (bold ? "Bold (on)" : "Bold") press=embolden retainFocus=true
      textarea id="editor" value=body input=edit select=selected markup="markdown"
      text body markup="markdown"
```

Keep `id` and `testId` separate:

- `id`: host command target, geometry, cross-node references.
- `testId`: driver/test target and stable inspection name. On the web it is
  `data-testid`, not the DOM `id`: a brief that asks for element ids wants `id=`
  (and `testId=` too where tests target the node).
- `each` key: data identity for row lifetime, not either node id.

Events bind actions (`press=save`, `input=edit(item.id)`). Captured arguments come
before host payload arguments. Do not add an event object or a JavaScript closure.
`navigate` is special: no captured arguments and zero or one location parameter.
`transformgeometry` and `transformrelease` must be paired. The complete payload
matrix is in [events](contract-grammar.md#events).

Known host commands are statements, not expression-returning functions. Several
commands have specialized argument checks; some generic commands leave validation
to the host. Do not infer that compiler acceptance proves a command's arguments
work. Use the command's working fixture and inspect its host result.

## Routes and web documents

Declare at most one `routes <slot>` in the app's root file. Each row names an
absolute path pattern, optional tab membership, parentage by indentation, and
optional rendering/activation policies. `notfound` is a bare fallback. Do not
redeclare the implicit router state with `state`.

Assign the results of `open`, `push`, `replace`, `go`, `select`, or `back` to that
router state. Use `path("route", args…)` to construct checked locations. A
template literal as a location is refused (`route-template`) and a string literal
is checked against the table, but any other computed string is not checked, so
always build locations with `path()`. A route's parameter fields are strings, with
empty strings for absent fields. `select` takes a tab name, not a URL.

`each entry in stack(nav) key=entry.id` gives retained screens their identities.
Bind the host's navigation root and per-entry `navigationKey`s as the
[router fixture](../contract/corpus/routes.contract) demonstrates. Test back,
tab switching, the same-URL push, external navigation, and route parameters.

Every route (a node with a `navigationKey` under the root) is a direct child of the
root or of a `role="tabpanel"` in it — through `each` and `when`, never another
element: the compiler refuses one behind a wrapper (`lower-route-place`). Make each
route `position="absolute" inset=0`: the hosts hide a covered route, as
`visibility: hidden` does, so an in-flow route still takes its room. A covered
route stays mounted (its nodes are in `tree`, marked `inactive`), so a `testId`
two screens share names two nodes: give each screen its own (`list-error`,
`detail-error`).

A route does not scroll by itself, as a `div` does not: content taller than it is
never seen. A fixed shell (a map, a camera, a chat whose composer stays put) is
fine; anything else puts its content in a `scroll` or `list` with `flex=1
min-height=0` right after its `header`, and names it with `navigationScroll`:

```text
column navigationKey=`${e.id}` navigationScroll="feed" position="absolute" inset=0 display="flex" flex-direction="column"
  header
    text "Inbox"
  scroll id="feed" flex=1 min-height=0
    …
```

`navigationScroll` names an element of the route by its `id`; on iOS that scroller
goes under the bar and collapses a large title, which happens only when it is the
route's child right after its `header`. The compiler refuses a literal name that no
element of the route carries, or one whose element is a box that never scrolls on y
(`lower-route-scroll`).

## Tabs and stacks

Tabs with a stack each have one layout that works on web, macOS and iOS
([the tabs fixture](../contract/corpus/tabs.contract), driven by
`host/web-js/conformance/tabs.steps`):

```text
main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow display="flex" flex-direction="column" height="100%"
  column flex=1 min-height=0 position="relative"
    each t in nav.tabs key=t.name
      column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
        each e in t.stack key=e.id
          column navigationKey=`${e.id}` position="absolute" inset=0 background-color="#fff"
            …
  row role="tablist" display=(top(nav).name == "full" ? "none" : "flex") height=56
    button role="tab" aria-controls="panel-home" aria-selected=(nav.tab == "home") press=pick("home")
      image "symbol:home"
      text "Home"
  when toast != ""
    button position="absolute" … // a root overlay: after the tablist, over everything
```

- Each tab names its panel with `aria-controls`; the panels are the stacks, and every
  tab's stack stays mounted, so a pushed screen, a draft and a scroll offset survive
  a visit to another tab. A tab is `select(nav, name)`; selecting the shown tab
  again pops it to its root.
- On iOS the panels become a `UITabBarController`: a tab of one symbol over its label
  is its bar item, a filled box holding a text is the item's badge, and the
  tablist's `accent-color` (inherited, as in CSS) tints the selected item. Under the
  agent the authored tablist and header paint and take taps instead; `tap tab-…` works on
  every host. To see and drive what a person sees, pass `--chrome platform` (a drive or
  a test on an iOS simulator): UIKit's tab bar, navigation bars and sheets show,
  `screenshot "file" window` draws them, and a tap naming a tab or a header control
  presses it through its bar (`native: "tab-bar-item"`, `"bar-button-item"`). The
  bars' own motion runs on real time (`clock +300 real` before a screenshot); the
  software keyboard, another window, is not drawn.
- Hide the tab bar on a route with `display="none"` on the tablist. Never remove the
  tablist with `when`: without it the root has no tabs and the panels' routes are
  found by no host.
- Root children after the panels box (a toast, a timer strip, a full-screen menu) paint
  over the routes and the native bars on every host, as later siblings do in CSS.
- A modal route (`navigationPresentation="modal"`) paints its own background; the
  route under it is dimmed.
- Without tabs, the routes are the root's own children, laid out the same way.
- Tests reach a tab by `tap`, or deliver a location as `type <root> "/saved"` (LLP
  1038 D11), which calls the root's `navigate`. On the web a CLI drive goes back as
  the browser's back button does with `tap <root> history -1`, `<root>` being the
  navigation root (the node with `navigationBack`) by its `testId`, the simplest
  handle, or its view number from `tree`; native hosts refuse it, and a
  test file has no such step yet.

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

`task hide when toast != "" key=toastUntil` with `after(5000, expire)` has its
timer only while the gate holds, as a `when` arm has its nodes, and a new key
restarts it, as a new `each` key makes a new row
([LLP 1092](../llp/1092-sends-that-queue-and-timers-that-wait.rfc.md)). Nothing
runs when the gate changes: turning true arms the timer from that commit's time,
turning false drops it, and an idle task keeps no host awake and commits
nothing at rest. `key=expr` alone means `when true key=expr`. An `after` fires
at its deadline exactly, so its action sees `now()` equal to the deadline: clear
without re-testing the time (a strict `now() > until` does nothing there). The
gate is a bool and the key a string, number or bool; neither may read `now()`
(`analyze-task-gate-clock`): gate on state and let the timer measure time. A
toast, a debounce (`when draft != saved key=draft` with `after(800, save)`), a
round's tick (`when screen == "play"`) and a flight's frames
(`when flying` with `every(frame, step)`) are each one gated task.

`now()` is the runner's clock in milliseconds since boot (the driver's clock under
the agent), not a date. For the date, read the reserved `exactTime` source and add
`time.epochAtZero + now()`. Its fields, which a shape declares as it reads them:
`epochAtZero` (Unix milliseconds when `now()` read zero), `utcOffset` (minutes east
of UTC), `locale` (BCP 47), `timeZone` (IANA), `resolvedLocale` (the language of
the string table the app shows, `""` with no tables) and `seed` (a whole number
drawn once per launch). A read does not itself schedule a future render, and a
derive that reads `now()` is not read again as time passes, and when it is read
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
which require optional capabilities. `spring(…)` (a `transition` timing
function), `exit-animation`, `layout-transition`, and presentation timelines have
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
handles of the types `file_handlers` declares. See
[Pointer](contract-grammar.md#pointer).

A long-press or right-click menu is a `popover` the node names with
`contextPopover="<id>"`: its `button` rows (with `popovertarget="<id>"
popovertargetaction="hide"`) and `hr` separators are the menu, and one row with
`contextPreview=true` is the preview, whose `press` is what tapping the preview
does (open the conversation). iOS presents UIKit's context menu, with the row
lifting and the preview popping into the screen its press pushes; macOS an
`NSMenu` without the preview; the web and the agent open the popover anchored to
the node. The node's own `contextmenu` action runs first, so one popover can
serve every row of a list. The agent opens it with `tap <node> contextmenu`
([LLP 1021](../llp/1021-menus.rfc.md) §5.1).

`frame(id)` and `measure("literal-id")` are action-only geometry reads returning
`Geometry` (`x`, `y`, `width`, `height`, `provisional`, `unavailable`). Handle `unavailable` and `provisional`. `frame` reads the last layout's border box
where the viewer sees it, as `getBoundingClientRect` does: in the viewport, through
every `translate`, `rotate` and `scale` on it and above it (the bounding box of a
turned box), with every scroll offset above it (the page's too) applied, so a drop
target needs no scroll bookkeeping and a dragged card is where it shows. Natively a
transform in flight counts at its end value; the web reads it mid-flight. `measure`
reads an auto-height hypothetical layout at the same origin.
Neither is a computed style binding to run every render.
`elementFromPoint(x, y)` names the front-most of the same boxes at a viewport
point by its nearest `id` (`option<string>`), through the same transforms. For a
drag still built by hand, test the dragged node's visual centre (the middle of
its `frame(id)`, which includes the `translate` the drag gave it), not the
pointer: the box under a lifted card is the card.

SVG uses SVG names. `foreignObject` compiles and renders on the web; native hosts
refuse it at run time, so position a box over the `svg` there. Canvas 2D calls
live in a data module, and GPU/game surfaces in their optional module. A
hyphenated native tag must be listed in `app.json`'s `modules` (the bake refuses
others) and implemented by the module; its unknown attributes pass through
unchecked as the module's props. A known attribute binds to the module's box, which
takes layout, box and paint rows, handlers, `testId`, `id`, `role`, ARIA, `disabled`
and `inert`; any other known name (`color`, `value`, `command`, `href`) is refused,
so give the module prop another name. Do not
turn a missing widget or canvas operation into invented Contract syntax.

Haptics are already there (LLP 1077 D14). `press-haptic` (`selection`,
`impact-light|medium|heavy|soft|rigid`) plays at touch-down without a round
trip, as `press-scale` does. `haptic("selection" | "impact-…" | "success" |
"warning" | "error")` is a host command an action runs, for example when a
drag crosses a threshold. iOS uses the feedback generators; the web vibrates
where it can; Linux does nothing.

Platform facts are reserved sources (`exactViewport`, `exactPage`, `exactDelivery`,
`exactSurface`, `exactTime`); the bake refuses a declared field the source does
not have. Use dimensions, media preferences, page facts,
and capability state rather than suffixing files by platform. Preference facts
inform authored policy; the engine does not automatically remove all motion.

Localized strings use `t("key", name=value)` and app `strings/<locale>.json`
files. Compile against the files to check keys and placeholders. Formatting
functions accept a narrow set of literal formats; app wording is an app `fn`.

## Inspection and testing

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
`"prefer prefers-reduced-motion reduce"`). Targets are `testId`s (or view ids): give every control a `testId` and
drive it on every host, iOS included (`agent ios`), never by screen coordinates.
A target no `testId` carries resolves by a view's exact accessibility label or
text (`tap "Save draft"`); a name several views share refuses, naming them.
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
contacts. The CLI forms are `tap world mouse` and `tap world contextmenu`, or use
a JSON options object for coordinates. `tap stage wheel 0 -20 modifiers Control`
is a pinch's wheel; `tap world drop a.board` drags a file in (web, macOS). Plain canvas taps and held contacts are
fingers, so their platform pointer identity and retained press history can differ
from a mouse's; use the intended physical input when comparing game saves.

`tap` and `type` scroll a target whose middle is out of view into it first (its
nearest scroll containers, then the page) and say so in the reply's `scrolled`.
A tap aims at the target's middle, or, where the target is not there (a wrapped
inline run, whose middle can fall between its lines), at the middle of the first
of its lines that is; a tap whose point lands on something else fails, an
ancestor that would take the press itself included.
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
focus leaving it), so an edit saved on `change` needs one of those. A CLI drive's
input does not wait for a request its action sends; `clock data` lands it.
`tap <target> drag <dx> <dy> … during "<op>" …` runs the quoted reads after the
move, with the finger still down, before the hold. Under `--touch platform` the
lift is scripted, so the reads run inside the hold and `during` needs one
(`hold 300 during "state"`). `tap <target> drag to <other> [at <x> <y>]`
ends the drag on another node (refused by name when it is not mounted or off
screen); a drag that should autoscroll a list or a board is `drag dx dy hold ms`.
During a reorder `state.reorder` reads `{ item, from, to, before, phase,
ending }`; a grouped grip also takes `type <grip> key Space` and the arrows. `clock +N` moves the virtual clock without
waiting for a store's or the network's reply on real time (unless a timer fires
first); its reply says what is still in flight (`inflight`, on every host), and `clock settle` lands it.
`clock settle` also runs the clock to where the last running finite, unpaused
transition or animation ends, firing the timers due on the way, so a test on a timer's grid moves with it.
`clock data` lands it without moving the clock: the data module's activation and
every request in flight, each answer's `then` with it, no timer fired. A CLI drive's
first operation runs at boot and may come before that has landed (an authored test
lands it before its first step), so a drive that reads or taps data starts with `clock data`.
A playing `video` or `audio` is on real time too: the clock never seeks or holds it, so
between operations it moves only as far as the drive took. `clock +N real` lets
N ms of real time pass with the clock moving beside it, a step at a time: a
video plays that far (its `timeupdate`s arrive), and a reply that lands in the
span lands (LLP 1042 §3).
Under the driver a picker, an export, a share or an auth session opens no panel:
it is held and `state` lists it under `pending` with its capability. Answer it by
the node its answer arrives at (the `id` the command names) or by its capability:
`type @folder-input path/to/folder`, `type @pick photo.jpg`, `tap @open-directory
cancel`. `@N`, its ticket, works too, but a ticket counts every request before it,
so it differs between hosts and runs.

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
names — written first in the test or, for every test, at the top of the file.
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
`times N` fails only the next N; `pass fetch "<prefix>"` stops it; a counted
fault that never fired fails the test. The app's own `catch`, error record and
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

A test whose text depends on the date names its `epoch`; without one it runs at
the driver's 2026-01-01 UTC. The steps are `tap "id" [hover|dblclick|contextmenu]`,
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
(`down`, `up`, or `for <ms>` on the virtual clock),
`type "id" paste "text"` (⌘V on macOS, Ctrl+V elsewhere, then the paste; a `key` handler that `preventDefault()`s that chord keeps it from landing), `type "id" copy`, `type "id" cut`, `pick "id" "path"…` or
`pick "id" cancel` (a held picker or export, by its node or capability as
above; paths are the test file's), `clock settle|data|+ms|+ms real|ms` (`data`:
what is in flight lands, with each answer's `then`, the clock unmoved), `resize
800x600` (the window, mid-test), `reload`
(the app restarts on the store it had, including a `secret.keep`, its state and
clock starting over, so a test shows what persists; the web reloads its current
URL, including its agent launch facts; a native app reopens at its launch
location), `close` (the window's close button, as ⌘W: a `beforeunload` that
calls `preventDefault()` keeps it open and the test goes on to the app's "Save
changes?"; macOS and the web),
`screenshot "file"`, `expect tree has|missing "id"`, `expect text "id" == "…"`
(the node's text; a control's value, so a `select` reads its chosen value, not its
options, and a checkbox with a `checked` binding `true` or `false`; else its descendants' — a button's label — else a field's value), and
`expect state name == <number|string|bool|none|[]>`, where `name` may go on into
a record's fields (`board.active.present`) or a list index (`rows.0`), and the
number may be negative (`== -3`). A failed expect with no input before
it names the requests still in flight (the boot's own, or what a `clock +N` left
on real time). An input step ends with what it settled: an answer the data
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
`aria-keyshortcuts="r"` button. A chord holds its modifiers for the key, in
Playwright's spelling: `"Shift+Enter"`, `"Meta+s"`, `"Control+Alt+ArrowLeft"`
([keys](contract-grammar.md#keys)). Not every interactive
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
| Component `contract` assertions | Authored `test` blocks exercised by the driver |
| `provide theme = value` around view children | Component `provide` section, wrapper component if needed |
| `items.map(...)` / `items[0]` | `map(items, …)` / `first(items)` or `at(items, 0)` |
| `map(items, x => Row(...))` | Keyed `each` with `Row(...)` in its body |
| `{ title: value }` | Declared record constructor |
| `[...xs, x]` / `xs.push(x)` / `xs.indexOf(x)` / `s.split(",")` | `concat(xs, [x])` / the same, assigned: `xs = concat(xs, [x])` / `indexOf(xs, x)` (or `includes(xs, x)` to test) / `split(s, ",")` |
| `{...old, title: value}` | `Shape(old, title=value)`, with the record's declared shape |
| `if name` for a string | `if name != ""` |
| `selected.title` when optional | Exhaustive `match selected` |
| `press={() => save()}` | `press=save` or `press=save(captured)` |
| Copying an action's statements into another | Call it: `arrive(path)`; the callee runs in the same commit |
| A helper that reads a slot its caller just assigned | Pass the value: `follow(next)` (`analyze-call-stale-read`) |
| Child `resource`, `mutation`, or `task` | Root-owned declaration and props/injections |
| `fontSize`, `radius`, `resizeMode` | `font-size`, `border-radius`, `object-fit` |
| `width=20px` | `width=20` or `width="20px"` |
| Unbounded `scroll` | Real height/max-height or flex in a bounded layout |
| `state item = none` with no usable type | Supply a typed use/write or rethink whether it is mutable state |
| Read a slot after writing it to get the new value | Compute `let next` before the assignments |
| `let next = …` where `next` is already an action, state or other name (`type-let-shadow`) | A name of its own: `let revised = …` |
| `let op = ""`, then `op = "tab"` in a branch (`type-let-reassign`) | Choose the value where it is bound: `let op = key == "Tab" ? "tab" : "insert"`; or a `state` |
| `mutation exported` and `action exported` (`type-duplicate-name`) | One name per declaration: a component's props, states, derives, resources, mutations and actions share names (`action fileSaved`) |
| Dynamic navigation template | `path("route", args…)` |
| Unconditional per-frame app work | CSS/presentation motion where possible; a frame task gated on the state that needs it (`task fly when flying`) |
| An always-on `every` that checks whether a toast expired | `task hide when toast != "" key=toastUntil` with `after(ms, clear)` |
| Add a function because it exists in JavaScript | Check the roster or put the operation in the data module; `len`, `split`, `push(xs, x)` and their kind are refused naming what to write |
| `background-color: "#fff"` in a `style` | `background-color="#fff"` |
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

CSS overflow accepts `visible`, `hidden`, `scroll`, and `auto`. `auto` clips and
permits scrolling, with indicators shown only when content overflows. A visible
axis beside hidden/scroll/auto computes to auto, as on the web.

`flex` accepts CSS `none`, `auto`, a basis, or `<grow> [<shrink>] [<basis>]`
(including `flex="0 1 auto"`). Numeric bindings keep the `n 1 0%` meaning.
Shorthands may be literal choices; computed strings are refused. A scroller
with `min-height=0` and positive shrink fits under a bounded flex column.

Dimension rows (width/height, their min/max, padding/margins, offsets and
border radii) accept `vw`, `vh`, `vmin`, `vmax`, and `svw/svh/lvw/lvh/dvw/dvh`.
On native, all viewport variants follow the window; on web, CSS resolves
small/large/dynamic viewports. Scalar lengths such as font size and gap do
not yet accept viewport units.

`translate` takes one or two lengths, each in px or a percentage of the box's own
border box, as CSS's does: `left="50%" top="50%" translate="-50% -50%"` on an
absolute box centres it, a percentage follows the box's size, and transitions and
keyframes interpolate the two parts as CSS does a `calc()`. `calc()` itself is
refused.

Transitions animate translate/scale/rotate/opacity, box paint (color,
background-color, border colors, tint-color, box-shadow), SVG paint/geometry
and the admitted numeric height path. `width` and other general layout
properties cannot interpolate yet: native layout is not run per frame.
The diagnostic names this engine limit; `layout-transition` animates a
change in the laid-out box using the existing measured projection.

`cursor` takes CSS cursor keywords (`pointer`, `grab`, `grabbing`, etc.) and
inherits. Web emits CSS; macOS maps to NSCursor with system artwork stand-ins
where needed; iOS/tvOS/Linux ignore the hint (LLP 1001). URLs are refused.

`font-family` accepts literal CSS fallback lists and choices of them, including
`"Inter, system-ui, sans-serif"` and quoted names. A family declared with
`font` uses its bundled faces; other names are local installed families,
whose own italic and bold faces `font-style` and `font-weight` select by CSS's
matching (a family without the face draws its nearest one, never a synthesized
slant or smear: LLP 1019 §5). Web and Apple retain the ordered glyph fallback cascade. Linux selects the first
installed family, then uses cosmic-text's platform glyph fallback; it logs
this declared limitation for a multi-member stack (LLP 1001).

A `text` with `selectionchange=act` hears the reader's real text selection
on the web and macOS: `action act(s: Selection)` gets `s.text`, `s.start` and
`s.end` in the node's own text ("" at 0, 0 when nothing there is selected).
Drive it with `tap <id> drag <dx> <dy> from <x> <y> mouse`.

Book typography is CSS's on every host. `text-align="justify"` fills all but
a paragraph's last line. `text-indent` is a length (`text-indent="1.5em"`,
`text-indent=24`; negative with the same `padding-left` hangs the first line).
`hyphens` is `manual` by default: a soft hyphen (U+00AD, written as the character itself or from data) breaks
and shows a hyphen; `none` ignores it; `auto` also hyphenates by the document's
language on the web and Apple (Linux has no dictionary and breaks only at soft
hyphens).

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

A bare text field (`input` of type `text`, `email`, `password`, `search`, `tel`,
`url`, `number` or none, and `textarea`) is visible, as the browser's is: a 1px
`light-dark(#c6c6c8, #48484a)` border, radius 6, padding 6/8, a
`light-dark(#ffffff, #1c1c1e)` fill and its own `light-dark(#000000, #ffffff)`
ink (it does not inherit `color`). These are rows under yours: any row or class
you write replaces that one row and keeps the rest; `padding` and `width` stay
content-box, so the field is 18px wider and 14px taller than its content.
`appearance="none"` (a literal) leaves them all out for a field you draw
yourself, such as a composer inside a pill (LLP 1104). A field in this look
shows a focus ring while focused (the web's `:focus-visible`, an accent ring on
macOS and Linux; iOS shows its caret) and dims to `opacity` 0.5 while
`disabled`; a bare field draws its own focus and disabled states.

`textarea rows=3` sets its preferred height in lines (default 2); explicit CSS
height and `field-sizing="content"` override it. `maxlength=80` on text inputs
and textareas limits user edits in UTF-16 units; authored `value` updates are
not truncated. It does not apply to `input type="number"`.

`resize="none"` disables browser resize handles. Other CSS resize values are
refused with a native geometry explanation. Given an action instead,
`resize=fit` is the element resize event, `ResizeObserver`'s: `fit` hears the
content box's width and height after the first layout and whenever they change
(one more parameter: its `DOMRectReadOnly`), on every host
([Events](contract-grammar.md#events)); read other boxes there with `frame(id)`
rather than polling with a timer. `user-select="none"` prevents
ordinary text selection; `auto` is the default. Text/all/contain need iOS and
Linux selection executors and are refused precisely. These rows take literals
or choices of literals, so unsupported runtime values cannot bypass the check.

A colour is any CSS colour the browser paints: hex, `rgb()`, `hsl()`, `hwb()`,
a named colour, `transparent`, `lab()`/`oklch()`/`color()` (clipped to sRGB
natively), or `light-dark(a, b)`; the kernel parses it once for every host.
`color-scheme="dark"` (or `"light"`) on a node makes that subtree resolve
`light-dark()`, platform colours and glass in that scheme, as a sheet that is
always dark does; leave it off to follow the surrounding scheme (LLP 1034 §8).
`currentcolor` takes the node's `color` on borders, `background-color`,
`tint-color`, text stroke and SVG paint. `unset` clears any row, and `inherit`
an inherited one (`color`, fonts, `fill`…); `inherit` on a row CSS does not
inherit is refused. `order` places flex and grid items. An image's accessible
name is `alt` or `aria-label`; `enterkeyhint` labels a soft keyboard's enter
key on the web and iOS. A bare number on a length row is pixels (except
`line-height`, where it is CSS's multiple of the font size), and the row takes
CSS's spellings too (`font-size="14px"`, `letter-spacing="-0.5px"`,
`padding="1.5rem"`; `stroke-width="2px"` but no `rem` there); `max-width` and
`max-height` take `none`, CSS's initial maximum, or `auto`, and the web writes
`none` for either. A number field's (`input type="number"`, written so)
`min`, `max` and `step` take numbers, as a range's do; its `value` is its text.

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
`off` clears the content type (the web's `autocomplete="off"`); `on` or a
list HTML's grammar refuses (the web's default) leaves `type`'s, and so does
a name the platform has no type for (`country`, `impp`, `sex`), which only the
web can act on.

`border`, `border-top/right/bottom/left` take CSS width/style/color in any order,
resetting omitted components to medium/none/currentcolor. Widths are px/pt,
unitless zero, or thin/medium/thick (1/3/5 px); styles are none/hidden/solid.
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
