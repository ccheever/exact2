# Start here: building an app with Exact

The only required reading; the rest is lookup.

## The idea

1. **Write the web, ship the platform.** Layout and names are CSS's and HTML's (flex, gap,
   `role`, `aria-*`, `inputmode`). Say what a thing *is* (a `header` with a heading and
   buttons, a `tablist`, a `dialog role="alertdialog"`); each host draws its own control.
2. **Leave the rest unsaid:** controls, bars, type and dark mode are the platform's. A
   colour or size names a role (`-exact-secondary-label`, `font="-exact-footnote"`; `bun
   <exact2>/scripts/no-tells.mjs .` lists literals). The one colour you write is the
   accent, on the root: a system colour (`accent-color="-exact-system-teal"`) or a
   `light-dark(light, dark)` pair. A card is `-exact-secondary-background`
   (`-exact-secondary-grouped-background` on a grouped page). White text on a prominent
   button reads on blue, indigo, purple, pink, red, brown, gray, not on green, mint, teal,
   cyan, yellow, orange (under 2.6:1: `check` refuses it; `RoundButton` takes black there).
3. **Design your content:** a focal point (big type, a fill, a shape), a hierarchy in
   rows, an empty state with its next step; inset-grouped is for settings and forms.
   `use X from "exact:content"` draws them (`none` for an option):
   - `RoundButton(label, tint, press, disabled, testId)`: Start, Stop
   - `AnswerTile(label, status, press, testId)`: a choice; `status` is `"idle"`,
     `"selected"`, `"correct"`, `"wrong"` or `"dimmed"`
   - `SummaryCard(label, value, detail, progress, symbol)`: the one number
   - `RingProgress` and `StatBar(label, value, fraction, tint)`
   - `RowText(title, detail, value)`, `Badge(label, tint)` a status, `DoneMark(caption, done)`
   - `EmptyState(symbol, title, message)` over your `button`
4. **A wide window** needs no code for tabs (a sidebar) or a list whose rows push (beside
   its detail); for two columns, wrap cards (`flex="1 1 320px"`) or `when viewport.width
   >= 900` over `resource viewport = exactViewport() as shape Viewport`.

## The workflow

```sh
bun <exact2>/scripts/exact.mjs new ~/groceries && cd ~/groceries
bun exact.mjs pattern      # lists the starters; `pattern <name>` writes one
bun exact.mjs ios --prebuild  # the first iOS build: minutes, in the background
```

|`bun exact.mjs …`|What it does|
|---|---|
|`contract build app.contract --json`|`[]` or each diagnostic with its fix; it rewrites the types `app.ts` imports.|
|`verify`, `verify ios`|Builds, runs `app.test.contract`, `check`s the first screen and where each test ends (light, dark, two widths), relaunches once on real time: one report. A clean run on the app's platform is done; a screen no test ends on goes unchecked.|
|`agent web --storage s1 "clock data" "tap add" "check both"`|Drives the app by hand, to see why something fails; without `--storage` it looks empty.|
|`web`, `ios --run`|For a person watching: its URL, rebuilds on save; a simulator.|

`verify`, `test web` (the tests alone) and `agent` build and serve their own copy: no dev
server is needed to check the app. Run `verify` after each edit, `verify ios` at
milestones; look at its screenshots (`.exact/verify/`). Keep scratch files in `.exact/`
(any other new file makes the build stale).

## One screen: the view, its data, its test

`app.contract` is the view, `app.ts` its data module, `app.test.contract` its tests. A
screen is a route: its `header` the iOS navigation bar over the `scroll` its
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
  resource groceries = loadItems() as shape list<Item> else empty()
  mutation added as shape Reply refreshes groceries then afterAdd
  mutation saved as shape Reply queue refreshes groceries
  derive left = length(filter(groceries, i => not i.done))
  derive addError = match added { case some(r) => r.error, case none => "" }

  action edit(v: string)
    draft = v
  action add
    if trim(draft) != ""
      send added = addItem(trim(draft))
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
            when addError != "" and not pending(added)
              text addError font="-exact-footnote" color="-exact-system-red" testId="add-error"
          button press=add disabled=(trim(draft) == "") testId="add"
            text "Add"
      section
        each i in groceries key=i.id
          row
            input type="checkbox" checked=i.done change=toggle(i.id) aria-label=i.title testId=`done-${i.id}`
            text i.title flex=1
        footer
          text (length(groceries) == 0 ? "Nothing on the list." : leftText(left)) testId="left"
```

- **Syntax.** Indent with spaces. Quote units (`width="50%"`); a bare number is pixels.
  Subtract with spaces (`a - b`): names hold hyphens. A view chooses with `when`/`else`,
  never `if`; no `form` (a field's Enter is its `submit`).
- **Expressions.** No truthiness. A record copy is `Shape(old, field=value)`. An event
  passes its captured arguments, then the payload (`toggle(i.id)` gets the bool).
- **Data.** A `resource` is a reactive read (`pending(x)`, `failed(x)`); a `mutation` a
  write (`queue` keeps them in order, `refreshes` re-reads, `then` runs an action: clear
  the field only on an ok reply, as `afterAdd` does). Only the root declares them;
  children take `props`.

**The data module** answers each source with exactly its declared shape (an extra field
fails it), reaches only what its `grants` name, and answers a failed fetch as data. The
clock and randomness are refused there; time is an argument.

```ts
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.groceries';
export const grants = 'fs.read app:/data/items.json\nfs.write app:/data/items.json';

type Item = Result<'loadItems'>[number];
const FILE = 'app:/data/items.json';
const load = (storage: Storage): Promise<Item[]> => storage.fs.readFile(FILE).then(
  (bytes) => JSON.parse(new TextDecoder().decode(bytes)),
  (e) => { if (e.code === 'ENOENT') return []; throw e; }); // no file yet ('bake', the build, passes through)
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

Steps: `tap "id"`, `type "id" "text"` or `type "id" key "Enter"`, `expect text "id" == "…"`,
`expect tree has|missing "id"`, `expect state a.0 .b == v`, `clock data` (lands replies)
`|settle` (ends transitions) `|+ms`, `reload` (same storage), `back`, `fail fetch "/p"`
and `pass fetch`, `request POST "/p" {…} as grace`; launch lines `size epoch time-zone
locale seed`. The rest: `tests`.

## Pitfalls that cost the most

1. **Styling a platform control.** Paint (`background-color`, `border`, `font-size`,
   `height`) on a `button`, field or bar makes it your box. Emphasis is a part (above) or
   `buttonStyle="bordered-prominent"`.
2. **A `header` holds a heading, one text after it (the bar's subtitle) and buttons**
   (or a search field, a `tablist`); anything else isn't in the iOS bar.

## Look it up

Open a recipe (`recipes/<name>.md`) when you write that part, not before: one per feature.

|The app has|Read|
|---|---|
|a second screen, list → detail, a delete alert|`pattern list-detail`; in an app you have, `navigation-stack`|
|tabs|`pattern tabs`|
|a summary or chart|`pattern dashboard`, `content-design`|
|its own server|`pattern rest`, `rest-backend`|
|a form in a sheet|`grouped-form`|
|a write that may be refused|`after-a-write`|
|a number or money field|`number-field`, `money`|
|a count 1–99|`contract vocab stepper`|
|dates, a day's records|`dates-and-days`|
|a timer|`timers`; still running after relaunch, `timer-that-survives-relaunch`|
|a saved setting|`persisted-setting`|
|randomness|`random-and-shuffle`|
|keyboard and focus|`keyboard-and-focus`|
|symbols|`symbols`|
|a chat thread|`chat-thread`|
|files or SQLite|`files-and-grants`|
|loading and errors|`fetch-loading-error`|
|a wide window|`web-look`, `responsive-columns`|
|driving iOS|`ios-driving`|
|functions|`functions`|

**Otherwise:** `bun exact.mjs contract vocab <name> …` answers any tag, property,
function, part or how-to (`screen`, `modal`); `docs/contract-for-agents.md`,
`docs/agent-pitfalls.md` and `docs/reference.md` by section; `<exact2>/apps/shelf` is a
whole app.
