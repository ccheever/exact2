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
| Complex native widget | App's declared native module |
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
Linux presenter. An iOS app's viewport is the device's screen, so iOS refuses it.
`--storage <name>` keeps a named scratch store between drives on every host (on
the web, a kept browser profile served on one port per name); without it a web
drive is a fresh profile, so its storage ends with the drive. That store includes
a Rust source's `secret.keep`: files in the named scratch tree on Apple and Linux,
and the page's `localStorage` on the web (the JS host always; the wasm page when
the drive names a store). A nameless drive keeps those secrets in memory only.
Firefox and WebKit open a fresh browser each drive, so a name there lasts for
the drive, including its `reload`. To show what survives a restart on any host,
use an authored test's `reload` step (below).

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

A script drives the same session in JavaScript: `const s = await open({ host:
'web', app, epoch, timeZone, storage })` from `scripts/agent.mjs`, then
`s.tap(target, opts)`, `s.type(target, text | { key })`, `s.clock(arg)`,
`s.tree()`, `s.state()`, `s.logs()`, `s.layout()`, `s.screenshot(path)` and
`s.close()` — the CLI's operations by the same names. `s.op(request)` is the
host's wire beneath them: it addresses views by numeric `id`, and it refuses a
request it would answer by doing nothing (a `target`, an unknown op, a web
`tap` with no browser input behind it). A reload or a raw browser step goes
through `s.carrier` (`evaluate`, and on Chrome `call` for CDP).

Use the existing five repository checks for repository changes. Do not add a
new global check or fixture framework for an ordinary app edit. For documentation,
compile complete examples, parse authored tests, and check local links.

## Language inventory

| Form | Placement | Meaning |
| --- | --- | --- |
| `use A, B as C from "./file.contract"` | File | Names from another file; nothing unnamed comes along (LLP 1091) |
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
| `mutation name as shape T` | Root component | Optional reply slot for explicit sends |
| `action name(args)` | Component | Event transaction; effects inferred |
| `task name mount` | Root component | One timer/frame schedule |
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
  `join` prints), builds strings of at most 64 MiB of UTF-8, and values of at
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
- Standard calls are free functions, not methods: `trim(s)`, `includes(s, q)`.
  There are no nonempty list literals, object literals, general lambdas, array
  indexing, assignment expressions, or JavaScript built-ins by implication.
- `fn` parameters and return types are explicit. Its body is one expression over
  its parameters and standard calls (including `now()`), without component-state
  capture or recursion. Pass an app value in; do not invent an ambient reference.
- Named arguments belong to component uses, record constructors,
  `t("key", placeholder=value)`, `empty(field=value)`, canvas `surface=` bindings,
  and the commands `share(…)`, `showNotification(…)` and `scrollIntoView(…)`. Every other function takes
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
a call of an action, `if`/`else`, and option `match`. No loops. The compiler
infers effects from the body, through its calls; `writes` is a refusal, not an
optional annotation. Use `symbols` when you need the inferred write set.

An action calls an action of its own component, an `action` prop or an injected
action by name, as a statement, anywhere a statement goes (LLP 1089). The call is
the callee's statements run where it stands, in the same commit: one rollback,
and the callee reads the state the action started with, as every statement does.
A name that is a host command stays the command. A call gives no value; compute
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
| Pending indicator | `pending(resourceOrMutationName)` |
| Resource request failed without an answer | `failed(resourceName)` |
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
request. Assigning a mutation forgets its in-flight reply, so an action that
sends one mutation twice on one path is refused (`analyze-send-twice`), counting
the sends of the actions it calls: send one combined request, or use a mutation
per request. `refreshes` re-reads
its resources when the mutation is sent (an answer the source gives at once shows
immediately) and forces them again when the reply lands. `then` is parameterless,
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
The [human guide's data-module section](contract-for-humans.md#writing-the-data-module)
has a complete `app.ts`: synchronous, `fetch` and SQLite sources, the grants
each needs, and how to drive it with storage.
The compiler accepting a source call does not provide its implementation. Check
its arguments, declared result, grants, storage access, and bake-time behavior.
Keep generated output out of version control. Use app-local sources for domain
formatting or algorithms beyond the finite standard roster.

A bake runs initial data work and packages first-frame values. A first-frame
value is not the answer: every host asks the TypeScript module again at launch,
natively once it loads after first pixel, even a source with no arguments
(`logs`: `<resource> shows its build-time answer until its source answers`, then
`<resource> answered: …`). Live requests run after that under host scheduling. Do not assume a secret store, disk database,
or authenticated network session is available while baking. See
[the data-module reference](reference.md#generate-typescript-data-source-types).

## Views, layout, and interaction

The view forms are element, component use, `when`/`else`, keyed `each`, exhaustive
option `match`, and `children`. Wrap root regions in a stable element. A component
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
or a growing `flex`, and takes `estimated-item-height`. A horizontal one needs a
literal `display="flex"` and a literal positive `height`, takes
`estimated-item-width`, and refuses wrapping, reversed or right-to-left flow, a
nonzero `gap`, main-axis padding, `justify-content` other than `flex-start`, and
`reorderdrop`. `reorderdrop` belongs only on a vertical `list virtualized=true`
(each row's handle names it with `reorderFor`); the compiler refuses it on any
other element, where no host could drag. Lists nest one level deep; an inner vertical list needs a literal
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

An `image` source is the same string on every host: a path under the app's
`assets/`, an `http(s)` URL, `symbol:<role>`, an `app:/data|cache|tmp/…` file
(a picked photo, or one the data module kept with `storage.fs`; it shows after a
relaunch too), or a `data:` URL of at most 1 MiB, past which every host shows
nothing (the web and Apple journal `image refused`). Keep a picked photo by copying it to
`app:/data` and answering that path; never tell hosts apart in the data module
(`HermesInternal`) to choose a source
([LLP 1069.002](../llp/1069.002-media-picker.rfc.md) D7, [LLP 1011](../llp/1011-image-v1.spec.md) §2).

A sound is HTML's `audio` (LLP 1042 §8): `video`'s props and events with no
picture, hidden unless it has `controls`. Bind `paused` and play from the input's
own action, so the play is inside the user gesture the web requires (a play that
nothing pressed for is refused, `error` `not-allowed`); mirror `pause` into the
binding, since a sound that ends pauses itself. Asking an ended sound to play
again starts it over, on every host:

```contract
component Ding
  state hush = true
  action ding
    hush = false
  action hushed
    hush = true
  view
    column
      button "Ding" press=ding
      audio "assets/ding.wav" preload="auto" paused=hush pause=hushed
```

Keep `id` and `testId` separate:

- `id`: host command target, geometry, cross-node references.
- `testId`: driver/test target and stable inspection name.
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
`visibility: hidden` does, so an in-flow route still takes its room.

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
  agent the authored tablist paints and takes taps instead; `tap tab-…` works on every
  host.
- Hide the tab bar on a route with `display="none"` on the tablist. Never remove the
  tablist with `when`: without it the root has no tabs and the panels' routes are
  found by no host.
- Root children after the panels box (a toast, a timer strip, a full-screen menu) paint
  over the routes and the native bars on every host, as later siblings do in CSS.
- A modal route (`navigationPresentation="modal"`) paints its own background; the
  route under it is dimmed.
- Without tabs, the routes are the root's own children, laid out the same way.
- Tests reach a tab by `tap`, or deliver a location as `type <root> "/saved"` (LLP
  1038 D11), which calls the root's `navigate`.

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
`every(frame, action)` entry. The action is parameterless. Millisecond intervals
are whole-number literals of at least 1. The frame form has no delta-time argument and does not
catch up missed display frames. For deterministic tests, use the driver's clock.

`now()` is the runner's clock in milliseconds since boot (the driver's clock under
the agent), not a date. For the date, read the reserved `exactTime` source and add
`time.epochAtZero + now()`. A read does not itself schedule a future render. Use a timer if a displayed value must keep changing without other
input. Prefer `clock settle` to waiting for a transition in real time.
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
node's content box, `buttons`, `pressure`, `pointerType`, `pointerId`), on any
node, a canvas included; set `touch-action="none"` on a drawing surface. Any
button goes down (`buttons` 2 is a right-click's), and a `contextmenu` action may
take the same record, where the click was. `wheel` hands a `WheelEvent` (deltas,
modifiers; a trackpad pinch is a Control-held wheel) and `preventDefault()` keeps
the scroll from happening; `drop` hands a `DragEvent` whose `files` are `doc:`
handles of the types `file_handlers` declares. See
[Pointer](contract-grammar.md#pointer).

`frame(id)` and `measure("literal-id")` are action-only geometry reads returning
`Geometry`. Handle `unavailable` and `provisional`. `frame` reads the last layout's border box
where the viewer sees it, as `getBoundingClientRect` does: in the viewport, with every
scroll offset above it (the page's too) applied, but untransformed, so a drop target
needs no scroll bookkeeping; `measure` reads an auto-height hypothetical layout at
the same origin.
Neither is a computed style binding to run every render.

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
commands. Use `tree` to find targets, `state` for data and delivery, `layout` for
geometry, `perf` for the work a drive cost (`perf <target> during "<op>" …`: per
plan site, evaluations, unchanged results, instances created and retired), and
screenshots for rendered output. Logs name refused operations and data errors.
For a game canvas, JavaScript `s.tap("world", {mouse:true, at:[x,y]})` sends one
primary mouse click on web, Windows, and Linux. `{contextmenu:true, at:[x,y]}`
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
`2026-10-09T14:30`), a range a number, a checkbox `true` or `false`.
`tap <target> drag <dx> <dy> … during "<op>" …` runs the quoted reads after the
move, with the finger still down. `clock +N` moves the virtual clock without
waiting for a store's or the network's reply on real time (unless a timer fires
first); its reply says what is still in flight (`inflight`, on every host), and `clock settle` lands it.
`clock data` lands it without moving the clock: the data module's activation and
every request in flight, each answer's `then` with it, no timer fired.
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
A test whose text depends on the date names its `epoch`; without one it runs at
the driver's 2026-01-01 UTC. The steps are `tap "id" [hover|dblclick|contextmenu]`,
`tap "id" modifiers "Shift+Meta"` (a press with keys held),
`tap "list" into "key"` (a virtualized list's row brought into view by its key,
so the next step can tap a row outside the rendered window),
`tap "id" pinch <scale> [at x y]` (two fingers; `scale` greater than 0),
`tap "id" drag dx dy [from x y] [mouse] [press ms] [over ms] [hold ms] [during "op" …]`
(`during` is last: quoted reads or `clock` while the finger is down, after the
move and before the hold; `press` and `hold` advance the virtual clock, except
under `--timing platform`),
`type "id" "text"` (sets the value), `type "id" "text" append` (after the value
the tree shows, as typing after a prefill), or `type "id" key "Name"`
(`down`, `up`, or `for <ms>` on the virtual clock),
`type "id" paste "text"` (⌘V on macOS, Ctrl+V elsewhere, then the paste; a `key` handler that `preventDefault()`s that chord keeps it from landing), `type "id" copy`, `type "id" cut`, `pick "id" "path"…` or
`pick "id" cancel` (a held picker or export, by its node or capability as
above; paths are the test file's), `clock settle|data|+ms|+ms real|ms` (`data`:
what is in flight lands, with each answer's `then`, the clock unmoved), `resize
800x600` (the window, mid-test), `reload`
(the app restarts on the store it had, including a `secret.keep`, its state and
clock starting over, so a test shows what persists),
`screenshot "file"`, `expect tree has|missing "id"`, `expect text "id" == "…"`
(the node's text; a control's value, so a `select` reads its chosen value, not its
options; else its descendants' — a button's label — else a field's value), and
`expect state name == <number|string|bool|none|[]>`, where `name` may go on into
a record's fields (`board.active.present`) or a list index (`rows.0`), and the
number may be negative (`== -3`). A failed expect with no input before
it names the requests still in flight (the boot's own, or what a `clock +N` left
on real time). An input step ends with what it settled: an answer the data
module gave in the input's turn, and its mutation's `then`, are there for the
next step. Otherwise the clock stands still between steps: a reply on real time
(a store's, the network's) or a transition an input started lands at a `clock`
step, so `clock settle` before the `expect` that depends on it. A timer
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
| `[a, b]` / `{ title: value }` | Source/list transform / declared record constructor |
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
| Dynamic navigation template | `path("route", args…)` |
| Unconditional per-frame app work | CSS/presentation motion where possible; bounded root frame task where needed |
| Add a function because it exists in JavaScript | Check the roster or put the operation in the data module; `len`, `split`, `push(xs, x)` and their kind are refused naming what to write |
| `background-color: "#fff"` in a `style` | `background-color="#fff"` |
| `change=flip(t.id)` on a checkbox, `action flip(id: string)` | The event appends its payload: `action flip(id: string, checked: bool)` (the refusal spells it) |
| Two `send`s to one mutation in one action | One combined request, or a mutation per request |

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
hyphens). `widows`, `orphans`, `break-*` and multi-column (`columns`,
`column-count`) are refused: nothing fragments a paragraph across boxes yet, so
page a fixed-height column by translating it (LLP 1001 §6). Multi-column and its
break rules are planned in LLP 1093 (admitted, not yet built); the refusals stay
until it lands.

`textarea rows=3` sets its preferred height in lines (default 2); explicit CSS
height and `field-sizing="content"` override it. `maxlength=80` on text inputs
and textareas limits user edits in UTF-16 units; authored `value` updates are
not truncated. It does not apply to `input type="number"`.

`resize="none"` disables browser resize handles. Other CSS resize values are
refused with a native geometry explanation. `user-select="none"` prevents
ordinary text selection; `auto` is the default. Text/all/contain need iOS and
Linux selection executors and are refused precisely. These rows take literals
or choices of literals, so unsupported runtime values cannot bypass the check.

A colour is any CSS colour the browser paints: hex, `rgb()`, `hsl()`, `hwb()`,
a named colour, `transparent`, `lab()`/`oklch()`/`color()` (clipped to sRGB
natively), or `light-dark(a, b)`; the kernel parses it once for every host.
`currentcolor` takes the node's `color` on borders, `background-color`,
`tint-color`, text stroke and SVG paint. `unset` clears any row, and `inherit`
an inherited one (`color`, fonts, `fill`…); `inherit` on a row CSS does not
inherit is refused. `order` places flex and grid items. An image's accessible
name is `alt` or `aria-label`; `enterkeyhint` labels a soft keyboard's enter
key on the web and iOS.

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
