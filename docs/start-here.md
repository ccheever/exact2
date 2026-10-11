# Start here: building an app with Exact

The only required reading (LLP 1115 D7). The rest is lookup: a short [recipe](recipes/)
per question (listed at the end), `bun exact.mjs contract vocab <name>` for any tag or
property, and the long guides by section only for what no recipe covers.

## The idea

1. **Write the web.** Layout is CSS (flex, gap, padding, `position`); names are HTML's
   and CSS's (`role`, `aria-*`, `inputmode`, `text-overflow`, `line-clamp`).
2. **Ship the platform.** Say what a thing *is* (a `header` with a heading and buttons,
   a `tablist`, a `dialog role="alertdialog"`) and each host draws its own control: on
   iOS the navigation bar, the tab bar, the system alert.
3. **Leave colours, fonts and metrics unsaid**, so dark mode, Dynamic Type and the tint
   come with the platform. To say one, name a role: `color="-exact-secondary-label"`,
   `font="-exact-footnote"`, `AccentColor`, `-exact-system-red`. A hand-built lookalike
   of a system control (a painted switch, buttons drawn as a tab bar, a title in a big
   `font-size`) is a bug; `bun <exact2>/scripts/no-tells.mjs .` lists literal values.
4. **Deliberate design is allowed.** The platform answers what you omit; the few things
   an app designs (a timer's big readout, a chart, a progress ring) may set their size
   and colour. Changing digits take `font-variant-numeric="tabular-nums"`.

## The workflow

```sh
bun <exact2>/scripts/exact.mjs new ~/groceries && cd ~/groceries
bun exact.mjs ios          # in the background now: the first iOS build takes minutes
bun exact.mjs contract types app.contract -o app.contract.d.ts
```

| `bun exact.mjs …`, from the app's directory | What it does |
| --- | --- |
| `contract build app.contract --json` | `[]`, or every diagnostic with its fix. After every edit. |
| `contract types app.contract -o app.contract.d.ts` | The types `app.ts` imports; rerun when a source's arguments or shape change. |
| `test web`, `test ios` | Builds, then runs `app.test.contract` (iOS after `bun exact.mjs ios`). |
| `agent web --storage s1 "clock data" tree "tap add" "screenshot .exact/a.png"` | Drives the app as a person would. Look at every screenshot. |
| `agent ios --storage s1 --chrome platform "clock +300 real" "screenshot .exact/s.png window"` | iOS as a person sees it, with UIKit's bars. |
| `web`, `ios --run` | The dev loop (it prints its URL, rebuilds on save); a simulator. |

Drive by `testId`, never by coordinates. Keep scratch files in `.exact/`: any other new
file is a source, and the driver calls the build stale.

## One screen: the view, its data, its test

`app.contract` is the view, `app.ts` the data module (each source the Contract calls,
and its `grants`), `app.test.contract` the tests. Start with one screen; this compiles:

```contract
routes nav
  home "/"

shape Item
  id: string
  title: string
  done: bool
  addedAt: number

shape Items
  items: list<Item>

shape Ack
  ok: bool

shape Time
  epochAtZero: number

fn leftText(n: number): string = n == 1 ? "1 item left" : `${n} items left`

component Groceries
  state draft = ""
  resource time = exactTime() as shape Time
  resource groceries = loadItems() as shape Items else empty()
  mutation saved as shape Ack queue refreshes groceries
  derive left = length(filter(groceries.items, i => not i.done))

  action edit(v: string)
    draft = v
  action add
    let title = trim(draft)
    if title != ""
      send saved = addItem(title, time.epochAtZero + performanceNow())
      draft = ""
  action toggle(id: string, done: bool)
    send saved = setDone(id, done)

  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="items" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
            text "Groceries" role="heading" aria-level=1
            button press=add disabled=(trim(draft) == "") testId="add"
              text "Add"
          list id="items" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                input appearance="none" value=draft input=edit submit=add placeholder="Add an item"
                  aria-label="New item" enterkeyhint="done" testId="draft" flex=1 min-width=0
            section
              each i in groceries.items key=i.id
                row
                  input type="checkbox" checked=i.done change=toggle(i.id) aria-label=i.title testId=`done-${i.id}`
                  text i.title flex=1
              footer
                text (length(groceries.items) == 0 ? "Nothing on the list." : leftText(left)) testId="left"
```

- **The screen.** A route's first child, its `header`, is the iOS navigation bar: a
  level-1 heading the large title, its buttons the bar items. A route doesn't scroll:
  `navigationScroll` names its `scroll` or `list`. `listStyle="inset-grouped"` draws
  `section`s as Settings does; a field in a grouped row takes `appearance="none"`.
- **Syntax.** Indent with spaces; `//` comments; attributes may continue on deeper
  lines starting `name=`. Quote units (`width="50%"`); a bare number is pixels.
  Subtract with spaces (`a - b`): names hold hyphens. Also `view`, `image "symbol:add"`,
  `textarea`, `select`/`option`, `scroll`, `dialog`, `progress`, `link`; no `form` (a
  field's Enter is its `submit`). A view chooses with `when`/`else`, never `if`.
- **Expressions** are free functions: `length(xs)`, `trim(s)`, `map`, `includes`,
  `first`/`at` (options), `concat(xs, [x])`, `match parseNumber(s) { case some(n) => …,
  case none => … }`. No truthiness. A record copy is `Shape(old, field=value)`.
  Events pass captured arguments, then the payload (`change=toggle(i.id)` gets the bool).
- **Data.** A `resource` is a reactive read (`pending(x)`, `failed(x)`); a `mutation` a
  write, which `queue` keeps in order and `refreshes` follows with a read. Only the
  root declares them; child components take `props`.

**The data module** answers each source with exactly its declared shape (an extra field
fails the resource). It reaches only what its `grants` name, one line each (`fs.write
app:/data/x.json`, `net.fetch https://api.example.com`), and answers a failed fetch as
data the view shows ([fetch-loading-error](recipes/fetch-loading-error.md)). The clock
and randomness are refused there (`Date.now()`, `Math.random()`, timers): time comes in
as an argument. `console.log` shows in the agent's `logs`.

```ts
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.groceries'; // exact new writes this line
export const grants = ['fs.read app:/data/items.json', 'fs.write app:/data/items.json'].join('\n');

type Item = Result<'loadItems'>['items'][number];
const FILE = 'app:/data/items.json';
let items: Item[] | null = null; // memory; the file is what survives a launch

async function load(storage: Storage): Promise<Item[]> {
  if (!items) items = await storage.fs.readFile(FILE).then(
    (bytes) => JSON.parse(new TextDecoder().decode(bytes)) as Item[],
    (e) => { if (e.code === 'ENOENT') return []; throw e; }); // no file yet; 'bake' (the build) passes through
  return items;
}
async function save(storage: Storage, next: Item[]): Promise<Result<'addItem'>> {
  items = next;
  await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify(next)));
  return { ok: true };
}

const sources: Sources = {
  loadItems: async (_args, _store, storage) => ({ items: await load(storage) }),
  addItem: async ([title, at], _store, storage) => {
    const all = await load(storage), id = String(Math.max(0, ...all.map((i) => Number(i.id))) + 1);
    return save(storage, [...all, { id, title, done: false, addedAt: at }]);
  },
  setDone: async ([id, done], _store, storage) => save(storage, (await load(storage)).map((i) => (i.id === id ? { ...i, done } : i))),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

**The tests** drive by `testId`, the same on the web and iOS. Each test starts with empty
storage, after the first data has landed:

```contract-test
test "an item is added, checked off and kept"
  expect text "left" == "Nothing on the list."
  type "draft" "Milk"
  tap "add"
  clock data
  expect text "left" == "1 item left"
  tap "done-1"
  clock data
  reload
  expect state groceries.items.0 .done == true
  expect text "left" == "0 items left"
```

`clock data` lands storage and fetch replies (after any input whose reply the next
`expect` reads); `clock settle` runs transitions to their end; `reload` relaunches on the
same storage. Every form, and the real-clock drive before calling it done: [tests](recipes/tests.md).

## The pitfalls that cost the most

1. **Styling a platform control.** A `background-color`, `border`, `font-size` or
   `height` on a `button`, field or bar makes it your box. Emphasis is
   `buttonStyle="bordered"` or `"bordered-prominent"`.
2. **`width="100%"` with padding overflows** (`content-box`, as in CSS). Drop the width;
   give a `flex=1` field `min-width=0`.
3. **Text in a `header` besides its heading and buttons** isn't in the iOS bar. Put a
   count or status in the content, and errors in words, never raw codes.
4. **A drive without `--storage <name>`** refuses storage, so the app looks empty.
5. **Trusting green tests.** They pass on a layout that spills off the screen, and on
   the virtual clock. Look at screenshots, iOS with `--chrome platform` and
   `screenshot … window`, and drive each write once on the real clock.

## Grow it: one line each, then its recipe

- Another screen, a sheet with Cancel and Save, a confirmation: [navigation-stack](recipes/navigation-stack.md).
- Tabs, when the app has at least two: [tabs](recipes/tabs.md).
- A form or settings screen, pickers, switches, validation: [grouped-form](recipes/grouped-form.md).
- A number field: `inputmode="decimal"` gives the iPhone's number pad: [number-field](recipes/number-field.md).
- A big number: its `font-size` with `font-variant-numeric="tabular-nums"`: [numeric-readout](recipes/numeric-readout.md).
- Money, `$1,481.47`: [money](recipes/money.md).
- Dates, day keys, "today": [dates-and-days](recipes/dates-and-days.md).
- Time that outlives a launch: store `time.epochAtZero + performanceNow()`; `performanceNow()` restarts at 0 on every launch: [timer-that-survives-relaunch](recipes/timer-that-survives-relaunch.md).
- Randomness: a seed from `crypto.getRandomValues` in a source: [random-and-shuffle](recipes/random-and-shuffle.md).
- A chat that stays at the newest message: `scrollFollowEnd=true`: [chat-thread](recipes/chat-thread.md).
- Icons: `image "symbol:add"` (a role), any SF Symbol as `symbol:sf/<name>`: [symbols](recipes/symbols.md).
- A remembered setting: [persisted-setting](recipes/persisted-setting.md).
- The network: [fetch-loading-error](recipes/fetch-loading-error.md); your own server: [rest-backend](recipes/rest-backend.md).
- Columns that wrap on a phone: [responsive-columns](recipes/responsive-columns.md).
- How it looks on the web: [web-look](recipes/web-look.md).

## Look it up, when no recipe covers it

| Topic | Where |
| --- | --- |
| Is a tag, attribute or property accepted | `bun exact.mjs contract vocab <name>` |
| Every test step and drive operation | [tests](recipes/tests.md); [grammar: authored tests](contract-grammar.md#authored-tests) |
| Every standard function | [grammar: standard functions](contract-grammar.md#standard-functions-and-intrinsics) |
| Any other declaration or its semantics | [contract-for-agents](contract-for-agents.md), by its contents |
| Compiles but lays out or behaves wrong | [agent-pitfalls](agent-pitfalls.md), by section |
| Data module globals, storage, grants, SQLite | [reference](reference.md#what-a-data-module-can-use); [human guide](contract-for-humans.md#writing-the-data-module) |
| Native code for what Contract can't say | [reference: access hatches](reference.md#access-hatches) |
| A whole iPhone app with tabs and tests | [`apps/shelf`](../apps/shelf/app.contract) |
