# Start here: building an app with Exact

The only required reading; the rest is lookup.

## The idea

1. **Write the web, ship the platform.** Layout and names are CSS's and HTML's (flex, gap,
   `role`, `aria-*`). Say what a thing *is* (a `header` with a heading and
   buttons, a `tablist`, a `dialog role="alertdialog"`); each host draws its own control.
   A data table is HTML's `table` (`thead`, `tr`, `th scope="col"`, `td colspan=2`), terms and their descriptions a `dl` (`dt`, `dd`).
2. **Leave the rest unsaid:** controls, bars, type and dark mode are the platform's, and so
   are a `list`'s rows: its children are rows, so draw no separator and no press. A
   colour or size names a role (`-exact-secondary-label`, `font="-exact-footnote"`; `bun
   <exact2>/scripts/no-tells.mjs .` lists literals). Leave the accent unsaid: it is the
   platform's blue, and a filled button keeps its white label (a pale accent under
   it is warned, `tamber-contrast`). A brand is an `app.tamber`; a colour names a role or
   one of its tokens, `color="ink"` or `color="var(--ink)"` (`recipes/tamber.md`). A card
   is `-exact-secondary-background` (`-exact-secondary-grouped-background` on a grouped page).
3. **Design your content:** a focal point (big type, a fill, a shape), a hierarchy in
   rows, an empty state with its next step in the list's place. A labelled entry or settings
   form is a grouped list (`grouped-form`), not cards of fields; a focal result (a timer, a
   converter's answer) stays a centred column (`max-width="26.25rem"`) or a `SummaryCard`.
   `use X from "exact:content"` draws them; the card gives each one's call (`none` for an option):
   - `RoundButton`: Start, Stop. `AnswerTile`: a choice; `status` is `"idle"`, `"selected"`,
     `"correct"`, `"wrong"`, `"dimmed"` or `"chosen"` (your pick once closed). `SummaryCard`: the one number, shown once
   - `RingProgress`, `StatBar`, `RowText`, `Badge` a status, `DoneMark` done or not; `EmptyState` with your `button` indented under it
4. **Compose as the platform does:** a row pushes its detail; create and edit are sheets; a
   phone gets one column, a wide window two (`responsive-columns`). Every screen, tab, detail
   and sheet is a route (`routes`, `push`, a row that is a `link href=path(…)`): on the web
   that is its URL, so Back, reload, a shared link and a new tab work, and focus moves to the
   new screen. A screen chosen by `if page == …` over a state has none of these, and the
   build warns (`lower-screen-by-state`). Tabs (a sidebar) and a
   list whose rows push (beside its detail) need no code. The rows of a list the person deletes from swipe to
   delete and open a menu on a long press (`list-detail`, `contract vocab swipe`).
   `haptic("selection")` marks a choice in your own control, `haptic("success")` a task done.
   A chat's messages are bubbles in a transcript that opens at the newest and follows each
   new one, not a grouped list's rows (`chat-thread`).

## The workflow

```sh
bun <exact2>/scripts/exact.mjs new ~/groceries && cd ~/groceries
bun exact.mjs pattern      # lists the starters; `pattern <name>` writes one
bun exact.mjs ios --prebuild  # the first iOS build: minutes, in the background; it names iOS in app.json
```

Start from the nearest pattern. The screen `exact new` writes, and its tests in
`app.test.contract`, are placeholders: replace both. `exact.mjs` runs from any
directory: a file you name is found where you are, else in the app, and it says
which it took. Your shell commands have no such help, so keep one directory.
|`bun exact.mjs …`|What it does|
|---|---|
|`contract build app.contract --json --errors-only`|A CI gate: `[]` and exit zero mean no errors (omit `--errors-only` to include warnings); it rewrites the types `app.ts` imports and type-checks it.|
|`verify`, `verify ios`|Builds, runs `app.test.contract` and each other `*.test.contract` beside it, `check`s the first screen and where each test ends (light, dark, two widths), relaunches once on real time, and reads those screens (on the web, also the ones the tests pass through) at the largest text size, where what spills is the app's layout to fix, with a remedy that keeps the text's size (no `line-clamp` in a list's row; `hyphens="auto"` only on a text of one word, which a UILabel breaks between syllables when it is longer than its line); `verify ios` also plays real touches on a long list on the first screen, once per build, and says `scrolling: like UIKit` or what a person would notice, each with UIKit's value and its cause, counting only what the app can fix: one report. A clean run on the app's platform is done.|
|`agent web --storage s1 "clock data" "tap add" "check both"`|Drives the app by hand, to see why something fails; `--keep-storage` reopens saved data.|
|`web`, `ios --run`, `windows --run`|For a person watching: its URL, rebuilds on save; a simulator; a window (from SSH, on the signed-in desktop).|

`verify`, `test` and `agent` compile, build and serve their own copy, saying a compile error as `contract build` does: no `contract build` before them, no dev server. `test linux` and `agent linux` do not build: run `bun exact.mjs linux` first and after each edit; nor do `test android` and `agent android`, which drive the APK `bun exact.mjs android` builds and installs on the device `ANDROID_SERIAL` names (`verify android` builds it). Run `verify` when the features work; `verify ios` or `verify android` once at the end for a phone app. On Windows, `test windows`,
`agent windows` and `verify windows` build first and drive headless at `app.json`'s `host.windows.window` size, in
DejaVu Sans with no frame; `verify windows` also captures the app as it opens on the desktop (`windows-window.png`):
judge the look from that one. Windows uses ink by default; `host.windows.painter: "cpu"` keeps the CPU window. `EXACT_PAINTER` overrides either choice. Headless drives explicitly use CPU unless overridden ([painter choices](reference.md#windows-painter)). A screenshot path with a space is quoted: `"screenshot 'C:\a b\x.png'"`. A screen you
want checked or captured is a test's end; a drive takes any others (`resize`, `prefer …`, `screenshot`).
Create seam types once with `bun exact.mjs contract types app.contract -o app.contract.d.ts`.
`contract build` refreshes that declaration after a successful compile and reports
the change on stderr. `contract types --check app.contract` checks freshness without writing.
`bun exact.mjs update` reports lock drift after this checkout moves; `bun exact.mjs lock`
refreshes it. `contract migrate app.contract` rewrites retired Contract syntax;
[the Snapback upgrade note](../snapback4/README.md#updating-an-inherited-app) covers
mutation refusals too.
Keep scratch files in `.exact/` and helper scripts you commit (a screenshot comparer) in
`tools/`: neither makes the build stale. Other files you add may (a Python helper under
`scripts/`, a JSON file `app.ts` imports). Nothing the app reads belongs in `tools/`: an edit
there never counts as a change.

**To check a screen against a reference** (the original's screenshot, a design), drive to it and
take `"screenshot .exact/home.png compare ref/home.png"`: it says in text, node by node, where
the two differ, and draws `home.diff.png`, so neither picture needs viewing.

`agent ios` is the app as a person sees it, UIKit's bars included, with screenshots a pixel a
point (`--scale 3`: the device's): no other simulator tool is needed ([ios-driving](recipes/ios-driving.md)).

## Look it up

Look up what the nearest pattern (a tested app) lacks:
`contract vocab` prints each name's answer in turn, a keyword, tag, function or part as well as a
recipe (`docs/recipes/<name>.md`, whole), so one call fetches what a screen needs:

```sh
bun exact.mjs contract vocab list-rows search swipe            # a list with search and delete
bun exact.mjs contract vocab for match persist grouped-form choice   # keywords, a settings form
bun exact.mjs contract vocab timers dates-and-days task        # a timer, a day's log
```

|Pattern|Already has|
|---|---|
|`one-screen`|a tracker: a total, quick-add buttons, a dated log in a file|
|`dashboard`|a summary, bars, dated rows, an add sheet that validates, two columns|
|`list-detail`|search, a detail, an add sheet, a delete alert, empty states, a file; split when wide|
|`tabs`|two tabs, a stack in each, a saved set kept with `persist`|
|`rest`|its own server, polled; a signed-in person (`rest-backend`, `fetch-loading-error`, `after-a-write`)|
|`site`|a website: a sortable `table` built ahead and filtered by a `select` in the query, a page per item rendered per request with a `dl`, a not-found page, a footer, an API's origin as a setting (`website`)|

|Not in a pattern|Look up|
|---|---|
|an entry or settings form|`grouped-form`|
|a sheet that edits the row tapped, filled with its values|`edit-sheet`|
|rows added and removed in a form|`editable-rows`|
|a write that may be refused|`after-a-write`|
|a failure the person must acknowledge (an alert with OK)|`error-alert`|
|tabular digits: numeric columns or values changing in place|`numeric-readout`|
|a number field, a stepper's `step`|`number-field`, `stepper`|
|a pick from six or more options (a picker)|`select`|
|a chat that opens at its newest message|`chat-thread`|
|a welcome at first launch|`first-launch`|
|records kept across launches, a saved library (`state` holds values, not records): a JSON file, SQLite, grants|`files-and-grants`|
|dice, a shuffle, a random pick or name (Contract has no `random()`: copy the recipe's `roll`)|`random-rolls`, `random-and-shuffle`|
|location, or anything the person must allow|`permissions`|
|another wide layout, a chart|`web-look`, `responsive-columns`, `content-design`|
|a video, a looping hero clip, a song (the `video` and `audio` elements, with a `poster`)|["Sound, MIDI and media"](contract-for-agents.md#sound-midi-and-media)|
|a map, the camera (drawing is boxes and SVG, not Swift)|`native-views`|
|a shared server (Snapback): wiring, tests, sign-in|`snapback`, `snapback-tests`, `snapback-auth`|
|sign in with Google, Microsoft, Auth0, Okta or another OAuth provider|`oauth`|
|a website: serving it (`serve`, which renders a `render=request` page per request), a setting, a deep link|`website`|
|ports, Bun, Chrome and its stores|`ports-and-browsers`|
|porting an app, a release build, helpers sharing a port|`porting-and-release`|
|a metronome, drum machine, sequencer or MIDI app: sound, MIDI, a transport (built in; write no native audio module)|`sequencer`; ["Sound, MIDI and media"](contract-for-agents.md#sound-midi-and-media); `apps/metronome`|
|transitions, keyframes, exit and layout animation|`motion`|
|brand colours, light and dark, icon tokens|`theme`, `tamber`|
|an icon: in a button, a tab, beside a label (a symbol, never a character such as ▦ ☏ ★ or an emoji)|`symbols`|
|a module in Swift that `app.ts` asks; launch arguments|`native-module`|
|app.json keys, `assets/`, the `modules` folder|`app-json`|
|a section of the long guides, whole, by its heading|`bun exact.mjs docs <heading words>`|

The others are named for their topic (`timers`, `money`…).

**A name in question:** `bun exact.mjs contract vocab` prints the card, or read `docs/card.md` (the same
text): every function, tag, part's call, test step, and how to do a screen, sheet, timer, colour,
data, test. A lookup is for a name the card lacks or a compile error;
`docs/contract-for-agents.md` and `reference.md` by section (`bun exact.mjs docs tabular numbers`
prints one with its path and line); `apps/shelf` is a whole app.

## One screen: the view, its data, its test

`app.contract` is the view, `app.ts` its data module, `app.test.contract` its tests. A
screen is a route: its `header` the iOS navigation bar (the Mac window's title and toolbar) over the `scroll` its
`navigationScroll` names (`one-screen`; more screens: `contract vocab screen`). A location's
fragment is the same screen: `/item/7#moves` opens at its `id="moves"` once its data has arrived,
and a `link href="#moves"` goes there. Every host, the web
too, hides a stack's covered routes itself (not the page under a sheet, nor a wide split's list
beside its detail); `for e in stack(nav)` keeps them mounted with their scroll, which `[top(nav)]`
loses. A query follows the path:
`` path("home") + `?q=${v}` ``. This list takes the scroll's place:

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
  an action does, and repeats with `for x in xs key=x.id` (`for x, i in xs key=x.id` binds the
  index `i` too); no `form` (a field's Enter is its `submit`).
- **Fields.** An `input` handler should update the draft on every edit so live search,
  validation and button enablement follow typing. A text field fires `change` on Enter or
  blur; its handler should normalize the draft. Use `submit` for explicit submission.
- **Expressions** are TypeScript's: what compiles means what it means in TypeScript, and
  what does not is refused with what to write. The web's string and array methods are
  methods (`s.trim()`, `xs.filter(x => …)`, `xs.length`); the rest are functions
  (`first(xs)`, `round(n)`). What differs: no truthiness; absence is an option (`none`,
  `some(x)`, `match`), not `null`; a value never changes (`xs.concat([x])`, a record copy
  `Shape(old, field=value)`); `==` compares lists and records by value; a stored number is
  finite (a `NaN` refuses the commit). A presentation order by one key is
  `sortBy(xs, x => x.name)`, stable (`"desc"` third; a second key is an earlier `sortBy`), not
  `sort`; grouping, joining and a stored or locale order are the data module's, and the view
  names the order it relies on where it reads it (`resource tasks = loadTasksByDueDate()`).
  Nothing runs because a value changed: an event runs an action, which passes its captured
  arguments, then the payload (`toggle(i.id)` gets the bool). Its writes land when it ends, so a
  later line reads the old value: bind the new one (`let n = go(nav, url)`, `nav = n`, read `n`).
- **Data.** A `resource` is a reactive read (`pending(x)`, `failed(x)`); a `mutation` a
  write (`queue` keeps them in order, `refreshes` re-reads, `then` runs an action: clear
  the field only on an ok reply, as `afterAdd` does). Only the root declares them;
  children take `props`. For a mutation without `queue`, the action that sends it must
  not also assign its slot to `none`: assignments commit after sends and can discard its
  new reply. Hide an old error with `pending(added)`. `none` means the slot contains no
  reply, not failure; use `failure(added)` to show request failures and match the reply
  to handle a domain refusal.
- **Refreshes.** Include every resource the write changes (`refreshes groceries, counts`).

**The data module** answers each source with exactly its declared shape: an extra
field fails a resource or voids a mutation’s reply, whose slot keeps its old value.
Type the function returning a reply (`Promise<Result<'post'>>`) so the build names
an extra field. It reaches only what its `grants` name. A value fixed at build, such as an
API's origin, is `const API = import.meta.env.API_BASE_URL` at its top level, the name declared
in `app.json`'s `env` ([rest-backend](recipes/rest-backend.md)). A failed read should
fail its resource so Exact can retain its previous answer; a mutation may return
a shaped domain refusal for its action to handle
([fetch-loading-error](recipes/fetch-loading-error.md)). The clock and
randomness are refused there; time comes as an argument. A date from a value
works (`new Date(ms)`, `Intl.DateTimeFormat`). `console.log` appears in the agent’s
`logs` and its last lines in a failed test’s notes. It imports its own `.ts`/`.js` and pure-JS npm
packages (ESM or CommonJS, a `browser` build first); one needing Node (`fs`, `Buffer`), the DOM or a native
addon is refused by name. Type an untyped one with `@types/<name>` or `declare module '<name>';` in a `.d.ts`.

```ts
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

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

Every step, launch line and `verify` flag is on the card.

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
   button there says `"bordered"`, and Cancel or Edit says nothing. A field on its own is
   the platform's bordered one; a document's own text (a note's title and body, as Notes
   draws them) says `border="none"`, and a chat's composer is `chat-thread`'s.
2. **A `header` holds a heading (`h1 "Title"`), one text after it (the subtitle) and
   buttons** (or a search field, a `tablist`); anything else isn't in the iOS bar. A
   control fixed under the bar goes with an inline title (`h2`); one that may scroll
   away, in the list's first section.
3. **`width="100%"` with padding overflows**, as in CSS: drop the width; a `flex=1` field
   takes `min-width=0`.

Consecutive `match` blocks in an action all run in order. Nest a match in the
selected arm, or use `if`/`else`, for exclusive paths. Action reads still see
the state from the action’s start.
