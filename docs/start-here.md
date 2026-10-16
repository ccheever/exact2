# Start here: building an app with Exact

The only required reading; the rest is lookup.

## The idea

1. **Write the web, ship the platform.** Layout and names are CSS's and HTML's (flex, gap,
   `role`, `aria-*`). Say what a thing *is* (a `header` with a heading and
   buttons, a `tablist`, a `dialog role="alertdialog"`); each host draws its own control.
2. **Leave the rest unsaid:** controls, bars, type and dark mode are the platform's, and so
   are a `list`'s rows: its children are rows, so draw no separator and no press. A
   colour or size names a role (`-exact-secondary-label`, `font="-exact-footnote"`; `bun
   <exact2>/scripts/no-tells.mjs .` lists literals). The one colour you write is the
   accent, on the view's root node: a system colour
   (`main accent-color="-exact-system-teal"`) or a `light-dark(light, dark)` pair. A brand is an `app.tamber`; a colour names a role or
   one of its tokens (`recipes/tamber.md`). A card is `-exact-secondary-background`
   (`-exact-secondary-grouped-background` on a grouped page). White text on a prominent
   button fails on green, mint, teal, cyan, yellow, orange (`check` refuses it;
   `RoundButton` tints them).
3. **Design your content:** a focal point (big type, a fill, a shape), a hierarchy in
   rows, an empty state with its next step; inset-grouped is for settings and forms.
   `use X from "exact:content"` draws them; the card gives each one's call (`none` for an option):
   - `RoundButton`: Start, Stop. `AnswerTile`: a choice; `status` is `"idle"`, `"selected"`,
     `"correct"`, `"wrong"` or `"dimmed"`. `SummaryCard`: the one number, shown once
   - `RingProgress`, `StatBar`, `RowText`, `Badge` a status, `DoneMark`; `EmptyState` over your `button`
4. **Compose as the platform does:** a row pushes its detail; create and edit are sheets; a
   phone gets one column, a wide window two (`responsive-columns`). Tabs (a sidebar) and a
   list whose rows push (beside its detail) need no code; one number and its buttons: a
   centred column (`max-width=420`).

## The workflow

```sh
bun <exact2>/scripts/exact.mjs new ~/groceries && cd ~/groceries
bun exact.mjs pattern      # lists the starters; `pattern <name>` writes one
bun exact.mjs ios --prebuild  # the first iOS build: minutes, in the background
```

For shared server data, `bun <exact2>/scripts/exact.mjs new app --snapback
[--port <n>]` from the backend’s directory writes a guestbook and wires its
driver, generated operations and device. The origin defaults to 4400.
`bun exact.mjs snapback` wires an existing app. With an existing schema, the
untouched greeting becomes a typed query and mutation explorer with `--online`.
`snapback --port <n>` records an ignored local backend selection.
`EXACT_BACKEND_ORIGIN` or `test web --backend <url>` selects the origin and matching
grant at bake time without editing authored source or tests. Password sign-in, stored sessions and refresh with Contract time are
in [the adapter guide](../snapback4/README.md#personas-and-password-accounts).
For a server-only schema, add `--online`; `snapback` and `update` preserve it.
On a plain scaffold, `snapback --port <n>` adds the origin and fetch/storage
grants. A computed origin or grants declaration needs an explicit edit.
After upgrading Snapback4, run `bun exact.mjs snapback --rebuild`.
`bun exact.mjs update` also rebuilds a mounted device after updating Exact.
Custom mounts: add `--artifact-only` to rebuild without rewiring.
The device's embedded `snapback4.engine` section records its identity;
`assets/snapback4.wasm.json` is derived diagnostic output.
`snapback`, `contract build`, `test web` and `agent web` refuse a stale device;
web drives compare it with the running server before opening the page.

Start from the nearest pattern. `exact.mjs` runs from any directory: a file you
name is found where you are, else in the app, and it says which it took. Your
shell commands have no such help, so keep one directory.
|`bun exact.mjs …`|What it does|
|---|---|
|`contract build app.contract --json`|`[]` or each diagnostic with its fix; it rewrites the types `app.ts` imports.|
|`verify`, `verify ios`|Builds, runs `app.test.contract`, `check`s the first screen and where each test ends (light, dark, two widths), relaunches once on real time: one report. A clean run on the app's platform is done.|
|`agent web --storage s1 "clock data" "tap add" "check both"`|Drives the app by hand, to see why something fails; `--keep-storage` reopens saved data.|
|`web`, `ios --run`|For a person watching: its URL, rebuilds on save; a simulator.|

`verify`, `test web` and `agent` build and serve their own copy: no dev server. Run `verify` when the features work; `verify ios` once at the end for an iPhone app. A screen you
want checked or captured is a test's end; a drive takes any others (`resize`, `prefer …`, `screenshot`).
Create seam types once with `bun exact.mjs contract types app.contract -o app.contract.d.ts`.
`contract build` refreshes that declaration after a successful compile and reports
the change on stderr. `contract types --check app.contract` checks freshness without writing.
Keep scratch files in `.exact/` (any other new file makes the build stale).

`agent ios` is the app as a person sees it, UIKit's bars included, with screenshots a pixel a
point (`--scale 3`: the device's): no other simulator tool is needed. `--touch platform` makes
each tap a real touch, which opens UIKit's menus and reaches a native module's own controls: drive
with it to compare a port's screens with the original's ([recipe](recipes/ios-driving.md)); porting
a React Native app, read [its pitfalls](agent-pitfalls.md#porting-a-react-native-app) first.
`--timing platform` gives UIKit its own timing, and `ios --release` builds
what ships (`--prebuild --release` early: its first build is minutes; `EXACT_UPDATE_TRUST=production
bun exact.mjs agent ios …` drives it).

**Ports.** `--port <n>` on `web`, `test web` and `agent web`, or `EXACT_PORT=<n>`,
serves on that port; a held port is refused. Otherwise each picks a free port within `PORT_LO`–`PORT_HI` when both are set, and
prints its URL. A native client starts a second loop on `n+1`. A storage store
keeps its data per port, so drive one store on one port.

- **Bun and Chrome.** Launchers prefer `$BUN`, then `~/.bun-1.4.2/bin/bun`,
  independently of a login shell’s PATH. Generated commands and child scripts
  inherit that binary. `./exact.mjs` also works when PATH has no Bun. Set `CHROME=/absolute/path/to/chrome` to select Chromium.
- **Named Chrome stores.** `agent web --storage s1` starts empty each run.
  Add `--keep-storage` to reopen its saved data and password session, or
  `--reset-storage` to explicitly reset it. A live drive owns its profile;
  another drive cannot reset it. Authored tests always isolate their browser
  stores, including across repeated suite runs. Backend rows persist: use
  `fresh` with `dev`, or create uniquely keyed fixtures with `serve`.
  Firefox/WebKit use a new profile each drive and do not preserve named stores.

**Splitting a port among helpers:** commit the seam between the halves first (the shared types,
the native module's interface), then start each helper in the background from that commit,
owning its half and stubbing none of another's. Keep test screens off branches you merge, and
keep working while they run.

## One screen: the view, its data, its test

`app.contract` is the view, `app.ts` its data module, `app.test.contract` its tests. A
screen is a route: its `header` the iOS navigation bar (the Mac window's title and toolbar) over the `scroll` its
`navigationScroll` names (`one-screen`; more screens: `contract vocab screen`). This list
takes the scroll's place:

```contract
shape Item
  id: string
  title: string
  done: bool

shape Reply
  ok: bool
  error: string

fn leftText(n: number): string = `${n} ${n == 1 ? "item" : "items"} left`

component Groceries
  state draft = ""
  resource groceries = loadItems() as shape Item[] else empty()
  mutation added as shape Reply refreshes groceries then afterAdd
  mutation saved as shape Reply queue refreshes groceries
  derive left = groceries.filter(i => not i.done).length
  derive addError = match added { case some(r) => r.error, case none => "" }

  action edit(v: string)
    draft = v
  action add
    if draft.trim() != ""
      send added = addItem(draft.trim())
  action afterAdd
    if addError == ""
      draft = ""
  action toggle(id: string, done: bool)
    send saved = setDone(id, done)

  view
    list id="items" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0 testId="app"
      section
        row gap=8 align-items="center"
          column flex=1 min-width=0 gap=4
            input value=draft input=edit submit=add placeholder="Add an item"
              aria-label="New item" testId="draft"
            if addError != "" and not pending(added)
              text addError font="-exact-footnote" color="-exact-system-red" testId="add-error"
          button press=add disabled=(draft.trim() == "") buttonStyle="filled" testId="add"
            text "Add"
      section
        for i in groceries key=i.id
          row
            input type="checkbox" checked=i.done change=toggle(i.id) aria-label=i.title testId=`done-${i.id}`
            text i.title flex=1
        footer
          text (groceries.length == 0 ? "Nothing on the list." : leftText(left)) testId="left"
```

- **Styles.** A colour behind a box is `background-color`; the `background` shorthand is refused.
- **Syntax.** Indent with spaces. Quote units (`width="50%"`); a bare number is pixels.
  Subtract with spaces (`a - b`): names hold hyphens. A view chooses with `if`/`else`, as
  an action does, and repeats with `for x in xs key=…`; no `form` (a field's Enter is its `submit`).
- **Expressions** are TypeScript's: what compiles means what it means in TypeScript, and
  what does not is refused with what to write. The web's string and array methods are
  methods (`s.trim()`, `xs.filter(x => …)`, `xs.length`); the rest are functions
  (`first(xs)`, `round(n)`). What differs: no truthiness; absence is an option (`none`,
  `some(x)`, `match`), not `null`; a value never changes (`xs.concat([x])`, a record copy
  `Shape(old, field=value)`); `==` compares lists and records by value; a stored number is
  finite (a `NaN` refuses the commit); sorting, grouping and joining are the data module's,
  and the view names the order it relies on where it reads it (`resource tasks = loadTasksByDueDate()`).
  Nothing runs because a value changed: an event runs an action, which passes its captured
  arguments, then the payload (`toggle(i.id)` gets the bool). Its writes land when it ends, so a
  later line reads the old value: bind the new one (`let n = go(nav, url)`, `nav = n`, read `n`).
- **Data.** A `resource` is a reactive read (`pending(x)`, `failed(x)`); a `mutation` a
  write (`queue` keeps them in order, `refreshes` re-reads, `then` runs an action: clear
  the field only on an ok reply, as `afterAdd` does). Only the root declares them;
  children take `props`.
- **Refreshes.** Include every resource the write changes (`refreshes groceries, counts`).

**The data module** answers each source with exactly its declared shape: an extra
field fails a resource or voids a mutation’s reply, whose slot keeps its old value.
Type the function returning a reply (`Promise<Result<'post'>>`) so the build names
an extra field. It reaches only what its `grants` name and answers failed fetches as data
([fetch-loading-error](recipes/fetch-loading-error.md)). The clock and randomness are
refused there; time comes as an argument. A date from a value works
(`new Date(ms)`, `Intl.DateTimeFormat`). `console.log` appears in the agent’s
`logs` and its last lines in a failed test’s notes.

```ts
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.groceries';
export const grants = 'fs.read app:/data/items.json\nfs.write app:/data/items.json';

type Item = Result<'loadItems'>[number];
const FILE = 'app:/data/items.json';
const load = (storage: Storage): Promise<Item[]> => storage.fs.readFile(FILE).then(
  (bytes) => JSON.parse(new TextDecoder().decode(bytes)),
  (e) => { if (e.code === 'ENOENT') return []; throw e; }); // no file yet ('bake', the build, passes)
const save = async (storage: Storage, items: Item[]) => {
  await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify(items)));
  return { ok: true, error: '' };
};

const sources: Sources = {
  loadItems: (_args, _store, storage) => load(storage),
  addItem: async ([title], _store, storage) => {
    const all = await load(storage), same = all.find((i) => i.title.toLowerCase() === title.toLowerCase());
    if (same) return { ok: false, error: `${same.title} is already on the list.` }; // a refusal is a reply too
    return save(storage, [...all, { id: String(all.length + 1), title, done: false }]);
  },
  setDone: async ([id, done], _store, storage) => save(storage, (await load(storage)).map((i) => (i.id === id ? { ...i, done } : i))),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

**The tests** drive by `testId` on every host, each from empty storage:

```contract-test
test "an item is added, a repeat refused, checked off and kept"
  type "draft" "Milk"
  tap "add"
  clock data
  expect text "left" == "1 item left"
  type "draft" "milk"
  tap "add"
  clock data
  expect text "add-error" == "Milk is already on the list."
  expect text "draft" == "milk"
  tap "done-1"
  clock data
  reload
  expect state groceries.0 .done == true
  expect text "left" == "0 items left"
```

Every step, launch lines (`epoch "…"`) and `verify`'s flags are on the card.

A Snapback app needs its backend running before `test web`: the runner checks
loopback origins in `app.ts`’s `net.fetch` grants and refuses a Snapback run if
none listens. Start `snapback4 dev` on that origin’s port, seeded if needed;
`EXACT_PORT` sets the page’s port and never moves the backend.

Each test opens empty app storage; backend rows survive tests and runs unless
`fresh` at the file’s top (or leading a test) empties `snapback4 dev` before each
test. This uses `POST /dev/reset` (Snapback4 **0.4.29**, `dev` only).
`fresh "seed.json"` then seeds it with `[{"table": …, "row": {…}}]`, from a path
relative to the test file. A server lacking reset fails the test.

Server ids are unique, not ordered. Sort by time; tests can reach rows by text
or position (`each t, i in tasks key=t.id`, ``testId=`task-${i}` ``).

Interactive web drives also accept `reload` (keeps storage and fetch faults) and
`pick <input-id> <path>`. `agent web --help all` lists driver capabilities.

Each mutation source request adds one millisecond of UI virtual time after the
step, including timer mutations issued during that advance. Reads and
continuations add none; arguments within an atomic action keep its clock
snapshot. The backend’s clock stays its own.

| To assert | Write | Not |
| --- | --- | --- |
| A node is there, or not | `expect tree has "<testId>"`, `expect tree missing "<testId>"` | `lacks`, `contains`, `not has`. It matches a `testId` only, never a label or text, unlike `tap` |
| A node's text | `expect text "<testId>" == "the whole text"` | `contains`, `!=`. The text is compared whole, and a node with no text of its own reads its descendants' joined: a row's testId reads its buttons' labels too, so put the testId on the text node |
| State | `expect state lib.notes.0 .title == "Milk"` (a space after a list index), `== []`, `== none`, `== true` | an expression on the right |
| A failed resource or mutation request | `expect text` on the view's `match failure(x) { case some(f) => f.code, case none => "" }`; `failed(x)` accepts either slot | `failure(x).code` without extracting the option. A thrown refusal is a request failure; an authored `{ok: false}` is a normal result |

## Pitfalls that cost the most

1. **Styling a platform control.** Paint (`background-color`, `border`, `font-size`)
   on a `button`, field or bar makes it your box. Emphasis is a part (above) or
   `buttonStyle="filled"` on the screen's main action. A `button` with no
   `buttonStyle` is the platform's: tinted text on iOS, so one that must read as a
   button there says `"bordered"`, and Cancel or Edit says nothing.
2. **A `header` holds a heading (`h1 "Title"`), one text after it (the subtitle) and
   buttons** (or a search field, a `tablist`); anything else isn't in the iOS bar. A
   control fixed under the bar goes with an inline title (`h2`); one that may scroll
   away, in the list's first section.
3. **`width="100%"` with padding overflows**, as in CSS: drop the width; a `flex=1` field
   takes `min-width=0`.

## Look it up

Start from the nearest pattern, a tested app with what the recipes teach. Open a recipe
(`recipes/<name>.md`) only when writing what the pattern lacks.

|Pattern|Already has|
|---|---|
|`one-screen`|a tracker: a total, buttons, a dated log in a file, a centred column|
|`dashboard`|a summary, bars, dated rows, an add sheet that validates, two columns|
|`list-detail`|a detail, an add sheet, a delete alert, a missing record, a file; split when wide|
|`tabs`|two tabs, a stack in each, a saved set kept with `persist`|
|`rest`|its own server, polled; a signed-in person (`rest-backend`)|

|Not in a pattern|Read|
|---|---|
|a settings form|`grouped-form`|
|a write that may be refused|`after-a-write`|
|a count 1–99|`contract vocab stepper`|
|SQLite, grants|`files-and-grants`|
|another wide layout, a chart|`web-look`, `responsive-columns`, `content-design`|
|a view only Swift can draw|`native-views`|

The others are named for their topic (`timers`, `money`…).

**A name in question:** `bun exact.mjs contract vocab` prints the card, or read `docs/card.md` (the same
text): every function, tag, part's call, test step, and how to do a screen, sheet, timer, colour,
data, test. A lookup is for a name the card lacks (`contract vocab <name> …`) or a compile error;
`docs/contract-for-agents.md` and `reference.md` by section; `apps/shelf` is a whole app.

Consecutive `match` blocks in an action all run in order. Nest a match in the
selected arm, or use `if`/`else`, for exclusive paths. Action reads still see
the state from the action’s start.
