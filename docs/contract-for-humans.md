# Contract: a guide for humans

Contract describes an application's interface, state, and interactions. Exact
compiles it for the web and native hosts. You write the view once; CSS names and
web behavior are the default across those hosts. Network access, persistent
storage, and substantial computation belong in a TypeScript or Rust data module.

This guide describes the implementation on `main` on 2026-10-02, including
immutable action locals, record construction, inferred action effects, and
component-level providers. The compiler and its tests are the authority.
Older Exact examples with `writes` clauses or component `contract` blocks no
longer compile.

Read this guide in order to learn the language, or use the contents as a reference.
The [agent guide](contract-for-agents.md) organizes the same language around
implementation and verification. The [grammar reference](contract-grammar.md)
collects syntax, operator precedence, built-in functions, tags, and events.

## Contents

1. [Run an app](#run-an-app)
2. [Your first component](#your-first-component)
3. [Files, indentation, and literals](#files-indentation-and-literals)
4. [Values and types](#values-and-types)
5. [State, derives, and actions](#state-derives-and-actions)
6. [Expressions and functions](#expressions-and-functions)
7. [Views and repeated content](#views-and-repeated-content)
8. [Components, providers, and slots](#components-providers-and-slots)
9. [Resources and mutations](#resources-and-mutations)
10. [Writing the data module](#writing-the-data-module)
11. [Styling and layout](#styling-and-layout)
12. [Input, events, and commands](#input-events-and-commands)
13. [Navigation and documents](#navigation-and-documents)
14. [Time, motion, and geometry](#time-motion-and-geometry)
15. [Graphics, media, and native extensions](#graphics-media-and-native-extensions)
16. [Platform facts and localization](#platform-facts-and-localization)
17. [Testing, diagnostics, and delivery](#testing-diagnostics-and-delivery)
18. [Where to go next](#where-to-go-next)

## Run an app

From an exact2 checkout with its pinned Rust toolchain and Bun installed:

```sh
bun install --frozen-lockfile
bun host/web/dev.mjs --app caltrain
```

Open the URL printed by the server, edit `apps/caltrain/app.contract`, and save.
The web build normally uses the JavaScript target; games use wasm. The build
reports unsupported features rather than silently selecting a different result.
Native builds require their platform tools. See the root
[quick start](../README.md#quick-start) for toolchain installation.

To scaffold a separate application, run from exact2:

```sh
bun scripts/exact.mjs new ../hello
```

The new app has `app.contract` (its interface), `app.ts` (its data module),
`app.json` (its manifest), `app.test.contract`, `web/` and `apple/` crates, and its
own Cargo workspace; it consumes this checkout by path. Its generated `exact.mjs`
runs the web and native commands (`bun exact.mjs test`, for example; `bun exact.mjs
test web tests/*.test.contract` runs the files named, from the current directory,
in turn); run it with no verb to list them, and see the [tooling reference](reference.md).
Stopping or killing `bun exact.mjs web` stops its dev server too.

You can compile a standalone Contract file without running a host:

```sh
cargo run -q -p contract -- build /path/to/app.contract --json
```

Below, `contract` stands for `cargo run -q -p contract --`.

A successful compilation is `[]` and exit status 0. It checks the language and
produces a plan; it does not prove that a data source is implemented or that an
app's measured layout works. The app build also bakes its initial data and layout.

## Your first component

This is a complete file. Save it as `app.contract` and compile it with the command
above. To run it, put it in a scaffolded app in place of its `app.contract`, delete
the scaffold's `greeting` entry from `sources` in `app.ts`, and replace
`app.test.contract` with the test under
[Testing](#testing-diagnostics-and-delivery).

```contract
component Counter
  state count = 0
  action increment
    count = count + 1
  action reset
    count = 0
  view
    column padding=24 gap=12
      text `Count: ${count}` testId="count" font-size=24
      row gap=8
        button press=increment testId="increment" padding=12
          text "Add one"
        button press=reset testId="reset" padding=12
          text "Reset"
```

The first component is the app's root. `state` holds a value, and an `action`
changes it. `view` describes the visible tree. Indentation makes the two buttons
children of the row. `press=increment` binds an action; it does not call that
action while building the view. A template puts the current count into text.

`testId` gives the driver a stable name for a node. It is separate from `id`,
which host commands and geometry functions use.

## Files, indentation, and literals

Top-level declarations start in column 1. Use spaces for indentation; two spaces
per level is the formatter's style. Blank lines and `//` comments are ignored.
There are no semicolons or braces around declaration bodies.

The top-level forms are `component`, `shape`, `fn`, `style`, `keyframes`, `font`,
`routes`, `use`, and `test`. A file can contain multiple declarations. Used files
may supply reusable declarations; the importing file's first component remains
its root.

```text
use Card, Badge as StatusBadge from "./parts.contract"
use Item, pulse from "./models.contract"
```

Each file has its own names (LLP 1091): its own declarations, and the names its
`use` lines list. Nothing comes along unnamed: if `parts.contract` uses `Icon`,
`Card` still works, but this file writes `Icon()` only after naming it too,
from `./icons.contract` or from `./parts.contract`, which passes on what it
names. The refusal names the lines a file lacks, and `contract fmt --uses
app.contract` writes them in every file of the app. `as` renames one name in
this file. Any component, shape, `fn`, style,
keyframes or timeline can be named; there is no `export` keyword.

Two files may declare the same name: each file's references mean its own
declaration. One name brought into a file twice from two files' declarations
(two however alike their text), or brought and also declared, is refused;
declare it once and `use` it from that file, or rename one with `as`. Fonts stay
app-wide, like CSS's `@font-face`.

A `use` names one of three things:

- a file of the app: `./parts.contract`, or `../lib/row.contract`, as long as
  it stays inside the app's directory;
- a built-in: `use Activity from "exact:motion"` is the one app-wide timeline
  for loading indicators, so every spinner keeps one rhythm;
- a package: `use Card from "@acme/ui"` reads an installed package's
  `.contract` files, found in `node_modules` as Node finds JavaScript. Its
  `package.json` `exports` maps `.` (and any subpaths) to `.contract` files;
  without `exports`, `index.contract`. A local library is a dependency too:
  `"@me/ui": "file:../ui"` in the app's `package.json`, or a Bun workspace.
  A package's own relative uses stay inside the package.

A library is Contract only: components, shapes, functions, styles, keyframes and
timelines. It cannot own resources or carry TypeScript; its components take data
through props, `inject` and slots. Fonts it names are the app's to supply.
Contract never imports JavaScript or TypeScript. Import cycles and unknown names
are refused. Keep external work behind the data interface.

Identifiers start with an ASCII letter or underscore. Letters, digits, and
underscores can follow. A hyphen followed by a letter is part of the identifier:
`background-color` is one name. Consequently, `a-b` is a name; write `a - b` for
subtraction. `x-1` is still subtraction, but spaces make the intention clearer.

| Literal | Examples |
| --- | --- |
| Number | `0`, `12`, `0.5`, `-8` |
| String | `"Hello"`, `"line\nbreak"` |
| Template | `` `Hello, ${name}` `` |
| Boolean | `true`, `false` |
| Empty option | `none` |
| Present option | `some(value)` |
| Empty list | `[]`, with its element type determined by context |

Use ordinary decimal numbers, double-quoted strings, and backtick templates.
Single-quoted strings, exponent notation, digit separators, `null`, and
`undefined` are not the language's literal forms. Strings accept exactly six
escapes: `\n`, `\t`, `\"`, `\\`, ``\` `` and `\$`. Template text is kept verbatim:
a backslash there is not an escape. Templates can contain expressions, quoted
strings, and nested templates. Keep an individual string or template on one
source line.

Newlines inside parentheses, brackets, and expression braces continue the same
logical line. Element attributes may continue on deeper-indented lines:

```text
text title
  font-size=24
  font-weight=700
  color="#243044"
```

An attribute continuation starts with `name=`. A child starts with a tag or a
component use. Use `contract fmt --stdout file.contract` to preview canonical
formatting, or `contract fmt file.contract` to write it.

## Values and types

Contract has closed types: `number`, `string`, `bool`, declared records,
`option<T>`, and `list<T>`. Component interfaces can also use the bare type
`action`. There is no authored `any`, nullable field shorthand, or arbitrary
JavaScript object. The compiler infers states and derives, but fields, component
props, injections, and function signatures declare their types.

```contract
shape Fields
  title: string
  pinned: bool

component Editor
  state fields = Fields(title="Untitled", pinned=false)
  action rename(value: string)
    fields = Fields(fields, title=value)
  action pin
    fields = Fields(fields, pinned=not fields.pinned)
  view
    column gap=8
      input value=fields.title input=rename testId="title"
      button press=pin testId="pin"
        text (fields.pinned ? "Unpin" : "Pin")
```

`Fields(title=…, pinned=…)` constructs a record and must supply every field
exactly once. `Fields(fields, title=value)` copies the base and replaces named
fields. It does not mutate the base. Fields are read with `fields.title`.
Only app-declared shapes can be constructed this way; compiler-owned shapes
such as `Router` and `Geometry` come from their designated operations.

Records, lists, and options are values; equality is structural. Shape definitions
must be finite and nonrecursive. Required record fields are never defaulted by
record construction. A resource's placeholder is a separate concept.

An `option<T>` is either `none` or `some(value)`. Unwrap it explicitly:

```text
derive selected = first(items)
derive caption = match selected {
  case some(item) => item.title,
  case none => "Choose an item"
}
```

The `some` branch's binding exists only in that branch. Both arms are required.
A state initialized to `none` needs enough information elsewhere, usually an
action's assignment of `some(...)`, to infer the element type. Actions and the
view see that type, but derives are typed first: match such a state in the view,
not in a derive. Similarly, `[]` needs an inferable list element type. A list
literal such as `[1, 2]` holds its items, which share one type; a list the screen
keeps for the session (a selection, open ids) is built in Contract, and a list the
app keeps across launches comes from the data module.

## State, derives, and actions

A `state` is stored. A `derive` is a computed expression over its dependencies.
Prefer a derive when a value can be calculated from existing data; there is no
reason to keep a second mutable copy of a filtered list or a display label.

```text
state query = ""
derive visible = filter(items, item => includes(item.title, query))
action search(value: string)
  query = value
```

Root components may own states, derives, actions, resources, mutations, and
tasks. Child components may own states, derives, and actions; root-only work
must be passed down as values or actions. State initializers read props,
injects and earlier states only: they run before any resource answers or
derive is computed. Derives can be declared in dependency order
or another order, but their dependency graph must be acyclic.

An action's state reads observe the state at its start. Its writes land together.
This is different from a sequence of imperative assignments in JavaScript:

```contract
component Snapshot
  state count = 0
  state observed = 0
  action step
    count = count + 1
    let before = count
    observed = before
  view
    button press=step testId="step"
      text `${count}/${observed}` testId="value"
```

After the first press, the text is `1/0`. The local `before` saw the old count.
To reuse a newly computed value, compute it once first:

```text
action step
  let next = count + 1
  count = next
  observed = next
```

`let` is immutable, belongs to its block, and is visible to following statements
and nested blocks. It cannot shadow an existing visible name, be reassigned, or
be read before its declaration. Separate branches may each declare their own
local of the same name. Locals do not become application state.

Actions support assignments to their own states and mutations, `let`, `send`,
`refresh`, host commands, calls of actions, `if`/`else`, and option `match`
blocks. They have no loops, `return`, or `await`. An action calls another action
of its component, an `action` prop or an injected action as a statement:
`arrive(path)` runs `arrive`'s statements right there, in the same commit, and
they too read the state the action started with. So a helper does not see what
its caller assigned before the call; the compiler refuses such a read and asks
for the value to be passed (`arrive(next)`). A name that is a host command, such
as `focus`, stays the command, and calling an action, prop or inject that
shares a host command's name is refused: rename it. A call returns nothing: share calculations through
`fn`. The compiler infers the state an action writes, through its calls. Do not
write a `writes` clause.

## Expressions and functions

Arithmetic, comparisons, boolean logic, field reads, and conditional expressions
work where values are expected. Conditions must be booleans: a nonempty string,
nonzero number, list, or option is not implicitly true.

```text
count > 0 and not loading
selected != none
length(items) > 0
busy ? "Working…" : "Save"
```

`and`, `or`, and `not` also accept `&&`, `||`, and `!`. Logical operators and
conditional branches short-circuit. Use parentheses around a composite node
attribute or positional expression to make its boundary obvious.

A file-scope function is one typed expression:

```contract
fn plural(n: number): string = n == 1 ? "item" : "items"

component CountLabel
  view
    text `3 ${plural(3)}`
```

Functions have no effects, cannot recursively call themselves or form cycles, and
do not capture component state (they can read `now()`). Pass values as parameters. Standard-function names
are reserved against redefinition. See the grammar reference for the complete
[standard-function list](contract-grammar.md#standard-functions-and-intrinsics).

Use free functions rather than methods: `trim(title)`, `includes(title, query)`,
`length(items)`, `first(items)`, and `at(items, -1)`. `first` and `at` return
options; `at` counts a negative index from the end. There is no `items[0]` syntax.

`map` and `filter` accept a callback with zero, one, or two parameters: the item
and its zero-based index. The callback is a single expression, reads its lexical
environment, and returns a value. `filter` requires a boolean. `join` accepts a
list of strings, numbers, or booleans and a string separator.

```text
derive names = map(people, (person, i) => `${i + 1}. ${person.name}`)
derive adults = filter(people, person => person.age >= 18)
derive caption = join(names, ", ")
```

Callbacks can build records. They cannot return view nodes. Use `each` to build
repeated UI, not `map`. General closures, reducers, sorting, and arbitrary list
mutation belong in the data module when the supplied operations do not suffice.

## Views and repeated content

An element has a tag, an optional positional value, named attributes, and
indented children. A component use has a capitalized name and named arguments
inside parentheses. The compiler checks tags, attributes, leaf-node children,
and handler signatures.

```text
text "Heading" font-size=24
image "/assets/photo.jpg" width=120 height=80 object-fit="cover"
Card(title="Heading", selected=selected)
```

Positional values depend on the tag. Prefer a corpus example when using a new
tag. The app's root view must be exactly one element: a `when`, `each`, or `match`
there is refused, so put a stable element around conditional or repeated content.
A child component used inside an element may return a region or several nodes.

`when` conditionally creates nodes:

```text
column
  when pending(results)
    text "Loading…"
  else
    text `${length(results)} results`
```

`match` renders an option's alternatives:

```text
match selected
  case some(item)
    text item.title
  case none
    text "Nothing selected"
```

`each` repeats a subtree, with an optional index and a required stable key:

```text
each item, i in items key=item.id
  row gap=8
    text `${i + 1}`
    ItemRow(item=item, open=open)
```

Keys identify rows across inserts, removals, and reordering. Use a unique durable
item identifier, not an index that changes when the list is edited. A child
component's state inside a row belongs to that row's key. Keep durable data in
the data module or retained app state; a removed row is not a persistent store.

For long lists, use a bounded virtualized collection:

```text
list virtualized=true height=500 estimated-item-height=64
  each item in items key=item.id
    column padding=12
      text item.title
```

A virtualized list has exactly one direct `each`, whose body has one flow root.
A vertical list needs a real height bound (`height`, `max-height`, or growing
`flex` in a bounded parent). A horizontal one needs a literal `display="flex"` and
a literal positive `height`, takes `estimated-item-width`, and refuses a nonzero
`gap` and `justify-content` other than `flex-start`. Padding along the list's
axis is room before the first row and after the last, as in CSS: a length or an
`env()` inset, not a percentage. On iOS a list's pull-to-refresh spinner draws
below its `padding-top`, so a header laid over that padding does not hide it. A
virtualized list's `scroll-padding` insets where `scrollIntoView` aligns a row,
as in CSS: `scroll-padding-top` the height of that header brings a row to just
below it, and the first row to the very top. Other elements refuse it.
Virtualized lists nest one level deep (an inner vertical list needs a literal
`height` or `max-height`); deeper nesting, masonry, wrapping, reversed lists, and
RTL horizontal collections are not supported.
`reachstart`, `reachend`, and `refresh` bind actions, optionally with captured
arguments, for fetching data. An estimate is a layout hint, not a data limit.

## Components, providers, and slots

A component declares its public inputs under `props`. Every declared prop must be
supplied exactly once, with a compatible type; props have no defaults. An action prop passes behavior
without giving a child access to its parent's state.

```contract
component App
  state chosen = ""
  action choose(value: string)
    chosen = value
  view
    column gap=8
      Choice(title="One", select=choose)
      text chosen testId="chosen"

component Choice
  props
    title: string
    select: action
  view
    button press=select(title) testId="choice"
      text title
```

For values needed throughout a component tree, use a component `provide` section
and a descendant's typed `inject` section:

```contract
component App
  provide
    accent = "#3355aa"
  view
    Label(title="Hello")

component Label
  props
    title: string
  inject
    accent: string
  view
    text title color=accent
```

A bare provider entry, such as `accent`, provides the existing value of that
name. A provider covers the component's entire view. The nearest providing
component wins. An unsatisfied injection is a compile error. `provide name =
value` inside a view is obsolete; use the component section shown above.

A slot allows a component to wrap caller-authored children:

```contract
component App
  view
    Card()
      text "Inside the card"

component Card
  slot
  view
    column padding=16 border-radius=12 background-color="#eeeeee"
      children
```

Declare `slot` on the receiving component and place `children` in its view.
The supplied children retain their caller's lexical scope and providers. This
is one slot, not named slots or an arbitrary render-function API.

## Resources and mutations

A resource is a reactive request to an app data source. The name reads as a value
of the declared type. When its arguments change, the runtime requests the new
answer. The source itself is implemented in TypeScript or Rust.

A mutation represents an explicitly requested operation. Its name reads as an
`option<T>`: `none` before an answer, `some(answer)` after one. `send` issues it;
ordinary assignment to `none` clears it and forgets an in-flight reply.

This complete Contract compiles; running it requires `loadItems` and `saveItem`
implementations in the app's data module:

```contract
shape Item
  id: string
  title: string
shape SaveResult
  ok: bool
  message: string

component Items
  state query = ""
  state notice = ""
  resource items = loadItems(query) as shape list<Item>
  mutation saved as shape SaveResult refreshes items then afterSave
  action search(value: string)
    query = value
  action save(id: string)
    send saved = saveItem(id)
  action afterSave
    match saved
      case some(result)
        notice = result.message
      case none
        notice = ""
  action retry
    refresh items
  view
    column gap=8 padding=16
      input value=query input=search testId="query"
      when pending(items)
        text "Loading…"
      when failed(items)
        button press=retry testId="retry"
          text "Try again"
      each item in items key=item.id
        button press=save(item.id) testId=`item-${item.id}`
          text item.title
      text notice testId="notice"
```

The `refreshes items` clause re-reads `items` when the mutation is sent (an answer
the source gives at once shows immediately) and forces it again when the reply
lands. `then afterSave` runs a parameterless action in its own commit at the
host's next clock advance, once for every answer that landed before it, so it
reads the latest answer (the agent driver lands it at the end of the input
that settled the answer). It does not run for a failure that brought no answer,
and it must not send its own mutation. Do not use `then` as a general event queue.

`pending(resourceOrMutation)` asks whether a request is in flight.
`failed(resource)` asks whether the current resource request failed without an
answer. Both take the declared name, not an arbitrary value. `failed` does not
accept a mutation: a mutation whose request fails without an answer stops being
pending and keeps its previous value, and its `then` does not run. A domain error
returned in a shaped answer is data to inspect, not a failed transport request.

Requests use newest-request-wins behavior; stale answers do not overwrite newer
requests. A failed resource keeps its retained value or placeholder and clears
pending. A changed argument or explicit refresh allows another attempt.

A write log wants every send, not the newest. Declare the mutation `queue`
(`mutation saved as shape SaveResult queue then afterSave`): one request is in
flight, and each later send waits, in order, with the arguments it was made
with, until the reply before it has landed and its `then` has run. The source
sees one request at a time, `then` runs once per reply, and `pending(saved)`
stays true while anything waits. Assigning `saved = none` forgets nothing under
`queue`: every reply still lands. Keep newest-wins for a sign-in or a draft whose
late reply should be dropped (LLP 1092).

A source sometimes needs current context to answer a question whose last answer
is still suitable to show. For example, a stored car status may need the current
minute to decide whether it must be fetched again:

```text
resource status = status(car) with minute as shape Status
```

The source receives `car`, then `minute`; changes to either ask again. `with`
accepts one or more expressions before `as shape`, and the call may be empty.
On hosts that persist eligible store-reading answers, a returning launch can
show the kept answer for the same `car` while the source is unready, even when
`minute` changed. It stays visible while activation is pending. Another car
shows the normal fallback. Without `with`, every argument must match. The default
web JS target keeps no persisted resource answers; see
[the platform scope](../llp/1027.005-resource-identity-and-request-context.rfc.md#d5--where-it-applies-and-the-gap-it-leaves).
The minute must already contain the current sample for the first ask; a timer
can update it thereafter. Use `refresh` or `refreshes` for values whose only job
is forcing another ask, and keep accounts, tenants and representation choices
among the call arguments.

Before an answer, a resource holds its baked value, its declared placeholder, or
its type's zero (`0`, `""`, `false`, `none`, `[]`, or a record of zeros):

```text
resource profile = loadProfile(id) as shape Profile else empty(name="Loading…")
resource profile = loadProfile(id) as shape Profile else previewProfile()
```

These are alternative declarations. `empty` supplies the type's zero, with named
constant field overrides for a record. A source-call placeholder takes plain-value
arguments, not state, and is answered once at build. See
[placeholder examples](../contract/corpus/placeholder.contract);
[RealWorld](../apps/realworld/app.contract) uses `else empty(…)`.

The app build bakes initial resource values into its plan for first paint. At
build there is no network, storage, store write or native module; a source that
needs a request is left unbaked and asked at run time. A baked value is only
the first frame: every host asks the TypeScript module again at launch (a native
host once the module loads after first pixel), even for a source with no
arguments, so what the module knows at launch reaches the view. `logs` names
each resource that showed a build-time answer and what its ask answered. Generate
the interface rather than guessing it:

```sh
cargo run -q -p contract -- types path/to/app.contract -o /tmp/app.contract.d.ts
cargo run -q -p contract -- rust path/to/app.contract -o /tmp/shapes.rs
```

Generated declarations are build artifacts. The data module's
`export const grants` governs network and storage permissions; a source name alone
grants nothing. The next section writes one end to end;
[Fieldnotes](../apps/fieldnotes) is a larger storage example.

## Writing the data module

The view asks; `app.ts` answers. The README's todo list keeps its data in memory.
This app keeps a reading list in SQLite, so it survives a restart, and shows a
quote fetched from the network. It was made with `exact new`, and its test passes
on the web host, macOS and the iOS Simulator.

```contract
shape Book
  id: string
  title: string
shape Saved
  ok: bool
  message: string
shape Quote
  text: string

component ReadingList
  state draft = ""
  state notice = ""
  resource books = books() as shape list<Book>
  resource quote = quote() as shape Quote else empty(text="…")
  mutation saved as shape Saved refreshes books then afterSave
  action edit(value: string)
    draft = value
  action add
    if trim(draft) != ""
      send saved = addBook(trim(draft))
      draft = ""
  action afterSave
    match saved
      case some(result)
        notice = result.message
      case none
        notice = ""
  view
    column padding=24 gap=12
      text "Reading list" font-size=28 font-weight=700
      text quote.text color="#6b7280" testId="quote"
      row gap=8
        input value=draft input=edit submit=add placeholder="A book" aria-label="New book" testId="title" flex=1
        button press=add testId="add"
          text "Add"
      text notice testId="notice"
      each book in books key=book.id
        text book.title testId=`book-${book.id}`
```

Generate the types `app.ts` imports, and regenerate them whenever a source's
arguments or declared shape change:

```sh
bun exact.mjs contract types app.contract -o app.contract.d.ts
```

```ts
import type { Answer, Database, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.reading-list';
// One capability per line: what this module may reach. Nothing else is allowed.
export const grants = [
  'sqlite.open app:/data/books.db',
  'net.fetch https://api.quotable.kurokeita.dev',
].join('\n');

// An open database locks its file, so a read and a write that overlap (a
// mutation and the refresh it triggers) would refuse each other as busy.
// One queue, one open at a time.
let queue: Promise<unknown> = Promise.resolve();
function withBooks<T>(storage: Storage, work: (db: Database) => Promise<T>): Promise<T> {
  const run = queue.then(async () => {
    const db = await storage.sqlite.open('app:/data/books.db');
    try {
      await db.execute('CREATE TABLE IF NOT EXISTS books (id INTEGER PRIMARY KEY, title TEXT NOT NULL)');
      return await work(db);
    } finally { await db.close(); }
  });
  queue = run.catch(() => {});
  return run;
}

const sources: Sources = {
  // A read: SQLite rows to the declared `list<Book>`. Integers arrive as
  // bigint, so convert them. At build (bake) time there is no storage: the
  // refusal's code is 'bake', and an empty list is the first frame's value.
  // The app asks again when it runs.
  books: (_args, _store, storage): Promise<Result<'books'>> =>
    withBooks(storage, async (db) => {
      const { rows } = await db.query('SELECT id, title FROM books ORDER BY id');
      return rows.map(([id, title]) => ({ id: String(id), title: String(title) }));
    }).catch((error) => {
      if ((error as { code?: string }).code === 'bake') return [];
      throw error;
    }),
  // A write, sent by the `saved` mutation; `refreshes books` reads the list again.
  addBook: ([title], _store, storage): Promise<Result<'addBook'>> =>
    withBooks(storage, async (db) => {
      await db.execute('INSERT INTO books (title) VALUES (?)', [title]);
      return { ok: true, message: `Added ${title}.` };
    }).catch((error) => ({ ok: false, message: String(error) })),
  // The network: only the origin `grants` names. A failure is data here.
  quote: async (): Promise<Result<'quote'>> => {
    try {
      const response = await fetch('https://api.quotable.kurokeita.dev/api/quotes/random');
      if (!response.ok) return { text: '' };
      const body = (await response.json()) as { quote?: { content?: string } };
      return { text: body.quote?.content ?? '' };
    } catch {
      return { text: '' };
    }
  },
};

export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

Each name the Contract calls (`books()`, `addBook(…)`, `quote()`) is a key of
`sources`. A source receives its arguments as an array, then the store (secrets),
`storage` (files and SQLite), and the native module, if any. It returns the
declared shape, or a promise of it. A resource and a mutation are answered the
same way; the difference is only who asks and when.

**Grants.** `grants` lists, one per line, everything the module may reach. A
call outside them fails. The capabilities are:

| Grant | Allows |
|---|---|
| `net.fetch https://api.example.com` | `fetch` to that origin (`https://*.example.com` for its subdomains). A redirect must stay inside the grants too; one that leaves them fails naming where it led: `outside the app's grants (net.fetch): redirected to https://other.example` |
| `net.websocket wss://api.example.com` | a WebSocket to that origin |
| `sqlite.open app:/data/name.db` | `storage.sqlite.open` on that path |
| `fs.read app:/data/dir`, `fs.write app:/data/dir` | `storage.fs` under that prefix (`app:/data`, `app:/cache`, `app:/tmp`) |
| `secret.keep name` | `store.keepKey(name, pair)` and `store.key(name)`: a P-256 key pair kept by the platform |
| `storage.kv scope`, `env.read NAME` | a host's key–value scope and environment variable (`grants/src/lib.rs`); a data module has no API for them yet: its `storage` is `fs` and `sqlite`, so keep key–value data in a file or a table |

**What catches people.**

- *A source cannot read the clock, start a timer or call `Math.random()`.*
  `Date.now()`, `new Date()` without a value, `setTimeout`, `setInterval`,
  `performance.now()` and `Math.random()` are refused when first used, on every
  executor (`crypto.getRandomValues` and `crypto.randomUUID` work inside an answer); the type check cannot see it, and only `logs` shows the refusal. Time
  and seeds are arguments: pass `now()` from the Contract (the
  [data-module reference](reference.md#generate-typescript-data-source-types) has the full list).
- *A source cannot open its own socket or request.* `new WebSocket(url)`,
  `XMLHttpRequest` and `EventSource` refuse on every host (`WebSocket is
  unavailable in data sources`), and every build refuses a direct use by file
  and line. Requests are `fetch`; a socket is read, never written, as
  `fetch("wss://…", { exactStream })` under a `net.websocket` grant.
- *There is no storage or network at build time.* The build bakes each
  resource's first value into the plan, and a storage call then is refused
  with `code: 'bake'`. An uncaught storage refusal leaves the resource unbaked;
  its placeholder (or the type's zero) shows until the app asks again at launch.
  Catching that code can also supply a first-frame value, as `books` does.
  Other source errors still fail the build. A source used as an `else` placeholder
  must answer at bake without storage. A `fetch` also stays unbaked; an `else`
  placeholder supplies something better than the type's zero, as `quote` does.
- *An open database locks its file.* A mutation and the refresh it triggers
  overlap, and the second `open` fails as busy. Queue every open, as
  `withBooks` does.
- *A save need not be awaited.* An editor answers from memory and saves in
  the answer, unawaited; the answer is there at once and the write finishes
  behind it, in the order the edits were made, on every host:

  ```ts
  edit(store, args) {
    song = apply(song, args);
    storage.fs.atomicWriteFile(PATH, new TextEncoder().encode(JSON.stringify(song))).catch(note);  // started now, not awaited
    return song;
  }
  ```

  Call storage in the answer rather than chaining it on a promise, keep a
  failure (`note`) to say in the next answer, and put writes that must stay
  together in one `transaction`.
- *SQLite integers are `bigint`.* Convert them (`String(id)`, `Number(n)`)
  before returning; a Contract `number` is not a `bigint`.
- *A domain failure is data.* `addBook` returns `ok: false` with a message
  rather than throwing, so the view can say what happened. A thrown error
  leaves a resource `failed(…)` and a mutation without an answer.
- *`app.ts` imports only local files.* npm packages are not bundled yet.

**Testing with storage.** Each authored test gets an empty store of its own,
apart from the app's real data. An ad hoc `agent` drive has none unless it
names a scratch store with `--storage`:

```contract-test
test "a book is added and kept"
  clock settle
  type "title" "Middlemarch"
  tap "add"
  clock settle
  expect text "notice" == "Added Middlemarch."
  expect text "book-1" == "Middlemarch"
```

```sh
bun exact.mjs test web                    # each test starts with an empty store
bun exact.mjs agent web --storage demo "type title Dune" "tap add" "clock settle" tree
```

A scratch store is kept between drives on every host, so a second drive with the
same `--storage demo` opens the list the first one saved: on the web, Chrome's
profile for that name, served at one origin (Firefox and WebKit drives start
fresh). A test's `reload` restarts the app on its store within one drive,
including a data module's `secret.keep`.

**Rust instead.** A data module can be a Rust crate rather than `app.ts`:
`bun exact.mjs contract rust app.contract -o shapes.rs` generates the shapes as
structs with their conversions, and [Caltrain's data crate](../apps/caltrain/data/src/lib.rs)
answers its sources that way. The [reference](reference.md#generate-typescript-data-source-types)
covers placement on a worker, live replacement, and the platform limits.

## Styling and layout

Use CSS names and values: `font-size`, `background-color`, `object-fit`,
`text-overflow`, and `line-clamp`. There are no React Native aliases such as
`fontSize`, `resizeMode`, or `numberOfLines`. A style attribute is `name=value`,
not a JavaScript style object.

A bare box uses CSS defaults, including `display: block`, `box-sizing:
content-box`, `flex-direction: row`, and `flex-shrink: 1`. `row` and `column`
are convenience flex containers. Do not infer flex behavior from arbitrary
parenthood, and do not assume border-box sizing. Set it when it matters.

Numeric dimensions normally mean pixels. Unit-bearing values and keywords are
strings: `width="50%"`, `height="auto"`, `padding-top="env(safe-area-inset-top)"`,
`width="calc(100% - 24px)"`, and CSS's `min()`, `max()` and `clamp()` over px,
the safe-area insets and viewport lengths: `padding-bottom="clamp(15px,
env(safe-area-inset-bottom), 60px)"`, `bottom="calc(max(15px,
env(safe-area-inset-bottom)) + 44px)"` (no percentage inside one: the kernel
resolves them before layout). Supported values are property-specific; this is not
an unrestricted browser stylesheet. The compiler and kernel reject unsupported
names or values. `line-height=1.5` is a ratio; `line-height="24px"` is fixed.

A reusable `style` contains literal style values:

```contract
style Panel
  padding=16 gap=8 border-radius=12
  background-color="#f0f2f5"

component App
  view
    column class=Panel padding=24
      text "A padded panel"
```

The element's own attributes override its class. `class=(selected ? Active :
Idle)` chooses between two named styles. This is not CSS selector matching, a
cascade, or a string of multiple class names. Put computed values on the node;
style bodies are constant. Properties omitted by the selected style revert to
their defaults rather than keeping the other style's previous value. The one
host-policy prop a style can hold, `buttonStyle`, must be set in both styles or
neither.

Useful property families include flex/block layout; sizes and min/max sizes;
padding, margin, and gaps; borders and radii; overflow and clipping; text metrics
and wrapping; color and gradients; transforms, shadows, filters, and animation;
and SVG presentation properties. The actual declaration inventory is
[`kernel/tables/schema.json`](../kernel/tables/schema.json), with authored names
and shorthands resolved by [`tags.rs`](../contract/lower/src/tags.rs). Use those
instead of assuming every CSS property or unit exists. A value is held to one
grammar on every host, a computed one too: `background-image` takes
`linear-`, `radial-` and `conic-gradient()` with percentage stops, so a
template naming `repeating-linear-gradient(` or `url(` fails the build, and a
computed value that is refused at run time is dropped and journaled on the web
as on a Mac (`invalid background-image value …; unset`), never painted by the
browser alone.

`backdrop-filter` accepts `none`, one `blur()` and/or one `saturate()` in the
order written. For example, `backdrop-filter="blur(12px) saturate(1.14)"`
blurs the backdrop, then increases its saturation. `saturate(0)` is grayscale;
`saturate(180%)` is the same as `saturate(1.8)`. A function may appear only once.
Web, macOS and Linux apply these functions; iOS/tvOS use the fixed `.light`
system material approximation. macOS samples only the parent layer's subtree
and clips children to the filter's border box. `backgroundMaterial` wins when
both are present, and backdrop filters do not animate.

Bound scroll containers. A typical full-height column gives its scroller
`flex=1 min-height=0`; an isolated scroller can use a numeric height. A scrolling
area as tall as all its children is not a usable scrollport. The bake checks
measured layout as well as the compiler's structural checks.

Use `aria-label`, roles, and other admitted ARIA attributes where content alone
does not name a control. Keep accessible labels separate from driver `testId`s.
They mean on every host what they mean in a browser: `aria-hidden` takes a
subtree off the tree and out of its ancestors' names; `role="checkbox"`,
`"radio"` or `"switch"` with `aria-checked` is that control, `role="img"` with a
label an image; `aria-labelledby` (the ids of the elements whose text names this
one, as a radiogroup names itself by its visible heading) wins over `aria-label`;
`aria-describedby` (the ids of the elements whose text describes
this one) and `aria-description` are its description. `aria-invalid`,
`aria-required`, `aria-haspopup` and `aria-current` (a navigation link's
`"page"`, a wizard's `"step"`) take their ARIA words or a bool; UIKit has no
property for those four, so iOS exposes none of them.
Font sizes, touch targets, focus behavior, and contrast remain author decisions.

Declare bundled fonts at file scope:

```text
font "Brand" = "assets/brand.ttf"
font "Body"
  400 = "assets/body-regular.ttf"
  700 = "assets/body-bold.ttf"
  400 italic = "assets/body-italic.ttf"
```

These are syntax examples requiring real font files. Apply with
`font-family="Brand"`. Respect the font's license and use the app's asset layout.

## Input, events, and commands

Bind a controlled text field's current value and its editing action:

```text
input value=query input=search placeholder="Search" aria-label="Search"
textarea value=body input=editBody
```

A text field is the platform's own by default (LLP 1104): `input` with no type
or `text`, `email`, `password`, `search`, `tel`, `url`, `number`, and `textarea`
outside the Markdown editor. On the web it inherits the page's font and
colour, as a CSS reset does. Disabled and placeholder appearances are the
platform's. A background, border or radius makes it your own box, as in a
browser; `appearance="none"` says so explicitly. A row on any conditional
class or value arm counts. `appearance="auto"` asks for the native field and
refuses those rows; `background-clip` and `background-attachment` are allowed.
Appearance is a literal, from the class then your own attribute; use `when`
with two fields to switch it.

`input` and `change` carry the control's new value as the final action argument:
a string for a text field, textarea or `select`, a boolean for a checkbox or
switch, the radio's `value` for `type="radio"`, a number for `type="range"`, and
a `list<Picked>` for a file input. An action taking one more parameter also
hears the target as the event leaves it, an `InputEvent`: its `value` (a
checkbox's own), `checked`, and a text field's `selectionStart`, `selectionEnd`
and `selectionDirection` ([form controls](contract-grammar.md#form-controls-radio-inputevent-setselectionrange)).
`hover` carries a boolean; `key` (keydown) and `keyup` carry a key name, and to an action that
takes one more parameter its `KeyboardEvent` (the modifiers, the physical key `code` and
whether it is an auto-`repeat`). Captured arguments precede
the payload: `input=edit(item.id)` calls the bound action with the id followed
by the new text. This syntax is binding, not immediate evaluation.

Use explicit types when they make the interface clear; omitted action parameter
types can be inferred from event sites. There is no inline `() => …` handler;
a `key` action claims its key with the host command `preventDefault()`, and
keeps it from its ancestors' `key` handlers with `stopPropagation()`
([keys](contract-grammar.md#keys)).

The complete event inventory and payload groups are in the
[event reference](contract-grammar.md#events). HTML controls include `select` and
`option`, and `progress` with no `value`, the platform's activity indicator
([activity](contract-grammar.md#activity-progress)); inspect [the control tests](../contract/cli/tests/it/controls.rs) for
the checkbox/switch, radio, range, select and date/time conventions instead of
assuming a browser Event object. `input type="radio"` is HTML's: the radios of
one `name` are a group, exclusive, and the arrow keys move the check among them.

An action can issue host commands such as `focus("editor")`,
`blur("editor")`, `copyText(text)`, and `openURL(url)`. Use an element's `id` for
commands that address a node. The [command inventory](contract-grammar.md#host-commands)
and linked checks describe the more specialized picker, sharing, formatting,
scrolling, and delivery commands. Commands are distinct from pure functions:
`openURL` does not return a Contract value, and a `fn` cannot issue it. The
compiler does not check most commands' arguments, and hosts differ: the
JavaScript web target has no `openURL`, and Linux carries neither `focus` nor
`openURL`. Check the host you target.

A file `input` needs a literal `accept`; types other than images and video must
be listed in `app.json`'s `file_handlers`. `showPicker` delivers a `list<Picked>`
to the addressed element's `change` handler, while `showOpenFilePicker`,
`showDirectoryPicker` and `showSaveFilePicker` deliver `doc:` handle strings;
cancellation uses `cancel`. Where a browser has no open pickers (Firefox,
Safari) they refuse with `cancel` too: read `exactPage().canOpenFiles` to tell
that from a person's dismissal and offer an import instead. A save there still
works: what the app writes to its handle downloads under the suggested name. File content, durable storage, and permissions belong
in the data module. See [file-picker syntax](../contract/corpus/file-pickers.contract).

On tvOS, `focusGuide="auto"` on a container guides a remote move entering its
box to the descendant that last held focus, or its first focusable descendant.
Moving within the container keeps UIKit's geometry. Other hosts ignore the
attribute and retain their normal focus order.

### Choosing a native button

An ordinary `button` is an authored box with `appearance="none"`. Opt into the
platform control with a literal `appearance="auto"`:

```contract
component NativeButtonExample
  state presses = 0
  action send
    presses = presses + 1
  view
    column gap=12
      button appearance="auto" buttonStyle="filled" press=send testId="send"
        text "Send"
      text `${presses}` testId="presses"
```

Its text and optional `image "symbol:…"` children describe the button's face;
they are not arbitrary layout children. A symbol-only face needs a nonempty
`aria-label`. The platform measures the control and supplies its chrome. UIKit
and AppKit use native controls, with stand-ins for styles a platform lacks; the
web and Linux draw their documented looks, which are not a promise of identical
glass rendering, and Linux draws no symbol image. A native button takes only
`press`, `focus`, `blur`, `key` and `hover` handlers.

`buttonStyle` defaults to `bordered`. The accepted styles are `plain`, `gray`,
`tinted`, `filled`, `borderless`, `bordered`, `bordered-tinted`,
`bordered-prominent`, `glass`, `prominent-glass`, `clear-glass`, and
`prominent-clear-glass`. This is a declared host-policy property, not a CSS
standard property. It can live in a style and can choose among checked literal
names. `appearance`, however, must resolve to a literal after class application;
use a view branch if switching between native and custom buttons.

Native buttons deliberately restrict authored paint, typography, face content,
and parent contexts so the platform can own the control. Do not transfer every
custom-button style to one. `accent-color` tints the styles that support it
(`gray`, `bordered`, `glass` and `clear-glass` ignore it). Follow
[the native-button fixture](../scripts/fixtures/native-buttons.contract) and
[its compiler checks](../contract/lower/src/controls.rs) for the admitted forms.

A settings screen is a grouped list: a `list` whose `appearance` is the literal
`auto`. Its children are `section`s. A section's first child may be a `header`
and its last a `footer`, each holding a `text`; everything between is its rows.
A row is read by its shape: an optional leading `image "symbol:…"`, a `text`
title, an optional second `text` (a value) or a `column` of two texts (a
subtitle), and an optional trailing accessory — a `forward-chevron` or
`checkmark` image, a checkbox or switch `input`, or a `button` holding only
`image "symbol:info"`. `destructive` draws a row red. Any other row is custom
and keeps its own views. A section written `background-color="transparent"` has no card: its rows sit on the list's background with no separators or corners, as a profile header does (LLP 1084 §6.2). That literal is the only `background-color` a section takes, and not beside a `class`; a section that gains and loses its card is two sections under `when`.

```text
list appearance="auto" listStyle="inset-grouped" flex=1
  section
    header
      text "Account"
    button press=openProfile
      image "symbol:person"
      text "Profile"
      image "symbol:forward-chevron"
    footer
      text "Who can see you."
```

`listStyle` is `inset-grouped` (the default), `grouped` or `plain`, a literal.
iOS draws UIKit's own list (`UICollectionView` with a list configuration); the
other hosts draw a sheet measured from it, and your own attributes replace any
of its rows. A section's own `margin-top` or `margin-bottom` is the space iOS
leaves there too, collapsed with its neighbour's as on the web (LLP 1084 §6.4).
See [the grouped-list fixture](../scripts/fixtures/grouped-list.contract)
and LLP 1084.

## Navigation and documents

A root-file `routes` table declares paths and an implicit router state. Nested
rows declare parentage; the path strings remain absolute patterns.

```contract
routes nav
  tab home "/"
    item "/item/:id"
  notfound

component App
  derive current = top(nav)
  action showItem
    nav = push(nav, path("item", "42"))
  action back
    nav = back(nav)
  action followLink(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${current.id}` navigationBack="back" navigate=followLink width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0 gap=8 background-color="#ffffff"
          text e.name testId=`route-${e.id}`
          when e.name == "item"
            text e.params.id
          button press=showItem testId=`open-${e.id}`
            text "Open item"
          button id="back" press=back testId=`back-${e.id}`
            text "Back"
```

A navigation stack is built this way: one row per entry of `stack(nav)`, keyed by
the entry's id, so a retained screen keeps its state. The root's `navigationKey`
names the top entry, and each row's `navigationKey` names its own; the host
presents the stack from them. `navigationBack` names the `id` of the back control.
Each row is a direct child of the root and fills it: a covered entry is hidden, not
removed, so one in the flow would still take its room. Tabs with a stack each are
laid out as [the tabs corpus](../contract/corpus/tabs.contract) shows.
`navigate=` receives locations the host navigates to itself, such as link clicks
and browser history.

`path("item", value)` checks the route and encodes its parameters. Always build
locations with it: a template literal as a location is refused and a string
literal is checked against the table, but any other computed string is not
checked. Parameter values can be strings or numbers. Declared route parameter
fields are strings; absent fields for another route are the empty string.

`push`, `open`, `replace`, `go`, `select`, and `back` return a router value; assign
the result to `nav`. `select` selects a tab by name. `top`, `stack`, and `depth`
read the router; `params(nav, name)` lists one parameter across the stack, and
`searchParam(entry, name)` reads an entry's query. Pushing the same URL already on
top does not add a duplicate visit. The [router corpus](../contract/corpus/routes.contract)
shows a full retained navigation stack and tab UI.

A `head` element declares document metadata: `title`, `description`, `canonical`,
`image`, `robots`, and `status` (a literal 404, 410 or 503). The innermost active
declaration wins field by field. Use `scroll document` for document scrolling and
ordinary `scroll` for an inner scrollport; both need a bound. Route rows can
declare `render`, `activate`, `paint` and `pages` policies; these interact with
the site's build/render pipeline, not just its client-side view. Follow
[the document corpus](../contract/corpus/document.contract),
[the document tests](../contract/cli/tests/it/document.rs) and
[LLP 1048.003](../llp/1048.003-documents-in-contract.spec.md) for accepted policy
values and serving behavior. A compiled interface alone is not a deployed website.

## Time, motion, and geometry

A root task schedules one action, with no arguments:

```contract
component Clock
  state ticks = 0
  action tick
    ticks = ticks + 1
  task ticker mount
    every(1000, tick)
  view
    text `${ticks}` testId="ticks"
```

`every(ms, action)` first fires one interval after boot, and `after(ms, action)`
fires once, `ms` after boot. Intervals are whole-number literals of at least 1.
`every(frame, action)` runs once per presented frame without catching up missed
frames. Each task body contains one schedule. Tasks are root-owned, not child
lifecycle hooks.

A task can wait for state instead of starting at mount:

```contract
component Undo
  state toast = ""
  state toastUntil = 0
  action deleted
    toast = "Deleted"
    toastUntil = now() + 5000
  action hideToast
    toast = ""
  task hide when toast != "" key=toastUntil
    after(5000, hideToast)
  view
    text toast testId="toast"
```

The timer exists while `toast != ""` holds, as a `when` arm's nodes do, and a new
`toastUntil` restarts it, as a new key makes a new `each` row: a replaced toast
gets its whole five seconds. Nothing runs when the gate changes, and an idle task
keeps no host awake. The action runs at the deadline exactly, so it clears the
toast without testing the time again. Gates and keys read state, never `now()`
(LLP 1092).

`now()` reads milliseconds since boot on the runner's clock (the driver's clock
under the agent); it is not a date. For the date, read the reserved `exactTime`
source and add `time.epochAtZero + now()`. Advancing the clock alone does not
necessarily trigger rendering: a derive using `now()` reevaluates when a later
commit evaluates it. Use a task when the display must tick.

Use CSS `transition` for changes to supported properties and `keyframes` with
`animation` for authored motion:

```contract
keyframes breathe
  from opacity=0.4
  50% opacity=1
  to opacity=0.4

component Motion
  state open = false
  action toggle
    open = not open
  view
    column gap=12
      button press=toggle testId="toggle"
        text "Toggle"
      column opacity=(open ? 1 : 0.4) transition="opacity 200ms ease" height=40
      text "Working" animation="breathe 1.6s ease-in-out infinite"
```

Keyframe values are literals or calls to the app's own `fn`s with constant
arguments (standard functions are refused), and keyframes animate paint and
transform properties, not layout ones such as `width`. Styles remain
literal-only. CSS easing and the admitted `-exact-spring(…)` timing function, which
belongs only inside `transition`, are not interchangeable guesses: copy the
appropriate [motion fixture](../contract/corpus/spring.contract).
In a `list virtualized=true` row, an animation waits until its row first
shows in the list (`-exact-animation-trigger="view"`, the default), because the list
builds rows before they scroll in; `-exact-animation-trigger="none"` starts it when
the row is built, so the row arrives settled. The web build does not hold it
yet.

`-exact-exit-animation`, `-exact-layout-transition`, and presentation timelines
(`-exact-drag-timeline`, `animation-timeline`, `animation-range`, `timeline-scope`) are
declared extensions with bounded behavior, not arbitrary layout animation.

For direct manipulation, `pan`, `panrelease`, `heightrelease`,
`transformgeometry`/`transformrelease`, and `reorderdrop` supply measured payloads.
The transform pair must be declared together. The height, transform and reorder
drags start only from a handle that names its target's `id` with `heightDragFor`,
`transformDragFor` or `reorderFor`, and `reorderdrop` belongs to a vertical
`list virtualized=true`, whose rows are the only ones a host can drag. The platform owns gesture
recognition and competition with scrolling; Contract does not define a general
gesture arena. See [Interaction Gallery](../apps/interaction-gallery/app.contract)
and [Spark](../apps/spark/app.contract) for complete bindings.

`frame("id")` reads the last laid-out border box where the viewer sees it, as
`getBoundingClientRect` does: in the viewport, every scroll offset above it
applied (the page's too), but without transforms. `measure("id")` asks for its
height-auto layout, at the same origin, under its
current offer; its id is literal. Both are action-only and return `Geometry`,
including `unavailable` and `provisional`; handle those flags rather than assuming
layout already happened. Geometry reads are not reactive view expressions.

## Graphics, media, and native extensions

SVG uses SVG's tags and presentation names: `svg`, `g`, `path`, `rect`, gradients,
masks, filters, text, and the rest of the admitted vocabulary. SVG `text` is
context-sensitive. `foreignObject` works on the web but is refused on native
hosts until a native box-in-scene implementation exists.

Canvas has two distinct providers: an app data module can record Canvas 2D calls,
and an optional GPU module can provide a surface. Contract declares the canvas
and its arguments; it is not a drawing-command language. Heavy computation and
game loops belong in those modules. See [Canvas Gallery](../apps/canvas-gallery/app.contract),
[SVG Gallery](../apps/svg-gallery/app.contract), and [the game workspace](../game/README.md).

`image`, `video`, `audio`, `iframe`, and Markdown-capable text/editors use host facilities.
Use `object-fit` for replaced media; distinguish text content from markup.
[Video Player](../apps/video-player/app.contract) shows playback bindings and
[Markdown Stress](../apps/markdown-stress/app.contract) selection and editing;
[Markdown](../apps/markdown/app.contract) is a reader. Markdown rendering and
editing run on the web and Apple hosts; Linux shows `markup="markdown"` text as
raw source and has no `iframe` or `video`.

A hyphenated tag can address the app's native module: the bake checks the tag
against `app.json`'s `modules` list, and its unknown attributes pass to the
module unchecked; a known attribute styles or labels the module's box, and one
the box has no use for is refused. Merely inventing a tag does not create a widget. Native
modules and GPU capabilities are separate optional artifacts; they do not add
features to every core build. Use [Photo Editor](../apps/photo-editor/app.contract)
as a concrete native-module example.

## Platform facts and localization

Host-owned facts arrive through five reserved sources: `exactViewport`,
`exactPage`, `exactDelivery`, `exactSurface`, and `exactTime` (the date, locale and
time zone). Declare the fields you read with their supported names and types;
the bake refuses a field the source does not have, though compilation alone does
not.

```contract
shape Viewport
  width: number
  height: number

component Responsive
  resource viewport = exactViewport() as shape Viewport
  view
    column
      when viewport.width >= 900
        text "Wide layout"
      else
        text "Compact layout"
```

`exactPage` answers the page's facts by the web's names: `visibilityState`
(`"visible"` or `"hidden"`), `onLine`, `canShare`, `canOpenFiles`, and
`hasFocus`, which is `document.hasFocus()`: true while the app's window has the
system's focus, false while another app or window is in front. An app that
tells the person something can show it in the window while it has focus and
post a notification otherwise.

```contract
shape Page
  hasFocus: bool

component Finished
  resource page = exactPage() as shape Page
  view
    main
      when page.hasFocus
        text "Build finished" role="status"
```

Prefer responsive branches driven by dimensions and actual capabilities to
inventing platform-specific Contract files. Safe-area lengths use CSS `env()`;
viewport metadata uses the root element's `viewport-fit` and `interactive-widget`
attributes. `exactViewport` also carries `prefersReducedMotion`,
`prefersReducedTransparency`, `prefersContrast` and `prefersColorScheme`: facts
for the app to honor, not automatic engine policy. Available fields are documented beside their source validation and
[the viewport design](../llp/1039-viewport-facts.rfc.md); start with [the viewport corpus](../contract/corpus/viewport.contract).

Localized strings live in the app's `strings/<locale>.json` files. Author calls
such as `t("greeting", name=person.name)`, with `{name}` in the table value. The
key must be a literal; keys and placeholder names are checked at compilation. The
base table is `en` unless `app.json` sets `strings.base`, and a translation may
omit keys but not add them. The locale comes from the host; lookup falls back from
the exact tag to shorter tags, then to the base. See
[the locale fixture](../host/web-js/conformance/locale) and
[the string tests](../contract/cli/tests/it/strings.rs).

`formatTime`, `formatDate`, and `formatNumber` are deterministic formatting calls
with specific accepted format literals. They print en-US whatever the viewer's
locale, and their vocabulary is not a general `Intl` options object. App-specific
wording belongs in app `fn`s.

## Testing, diagnostics, and delivery

Put authored interaction tests in `app.test.contract`, beside the app:

```contract-test
test "the counter increments"
  expect text "count" == "Count: 0"
  tap "increment"
  expect text "count" == "Count: 1"
  expect state count == 1
  tap "reset"
  expect state count == 0
```

These steps exercise the first example. Parse a test file with `contract test`;
run it against a built app with the driver:

```sh
cargo run -q -p contract -- test path/to/app.test.contract
bun scripts/agent.mjs web --test path/to/app.test.contract
```

Use the correct `--app`, `EXACT_APP_DIR`, and host build for your application;
without either, the driver uses Caltrain. The driver refuses stale artifacts and
prints the needed rebuild. `clock settle` advances animation deterministically
(it reports `settled: false` while a repeating timer or an infinite animation
runs, as in the Clock and Motion examples); `clock +1000` advances by a second.
Do not replace virtual time with sleeps.

For diagnosis, use `build --json`, `symbols --name <name>`, and
`build -o /tmp/app.plan --map`, which writes `/tmp/app.plan.map.json`. The
development source map connects node layout
to declarations, component calls, and style origins. Then drive the actual app:

```sh
bun scripts/agent.mjs web tree state logs "screenshot /tmp/app.png"
```

Compilation, baking, and interaction are different checks. If a view is blank,
inspect `tree`, `layout`, resource state, and logs before changing random styles.
If a resource fails, check the source implementation, its generated types and
grants, and its bake behavior. If an event fails, check the captured arguments,
payload type, current action binding, and whether the node exists.

`app.json` also controls delivery policy. `bun scripts/deploy.mjs <app>` is a
dry run, which expects a committed tree (or `--dirty`); publication uses `--yes`,
an origin, a signing key, and the configured signed streams. Read
[LLP 1030.000](../llp/1030.000-dev-server-as-deployer.rfc.md) before changing
trust or publishing. Writing Contract does not require inventing a deployment
service.

## Where to go next

| Need | Working source |
| --- | --- |
| Small stateful app and tests | [Caltrain](../apps/caltrain/app.contract) |
| Record updates and locals | [records](../contract/corpus/records.contract), [let](../contract/corpus/let.contract) |
| Providers and caller-owned slots | [provide](../contract/corpus/provide.contract), [slot](../contract/corpus/slot.contract) |
| Lists | [lists](../contract/corpus/lists.contract) |
| Date and time formatting | [Caltrain](../apps/caltrain/app.contract) |
| Requests, storage, editing | [Fieldnotes](../apps/fieldnotes/app.contract) |
| Authentication and redirects | [RealWorld](../apps/realworld/app.contract) |
| Navigation | [routes](../contract/corpus/routes.contract) |
| Motion and direct manipulation | [Interaction Gallery](../apps/interaction-gallery/app.contract) |
| Text flowing around shapes | [Text Flow](../apps/textflow/app.contract), [Reflow](../apps/reflow/app.contract) |
| All language forms | [grammar reference](contract-grammar.md) |

Read current source examples rather than predecessor design documents. A feature
in `llp/research/` is historical evidence, not proof that the current compiler
accepts it. When the compiler refuses something, keep its diagnostic id and
source range: its suggested accepted form is often the quickest next step.
