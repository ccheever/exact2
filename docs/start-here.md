# Start here: building an app with Exact

The only required reading; the rest is lookup, listed last.

## The idea

1. **Write the web.** Layout is CSS (flex, gap, padding, `position`); names are HTML's
   and CSS's (`role`, `aria-*`, `inputmode`, `text-overflow`).
2. **Ship the platform.** Say what a thing *is* (a `header` with a heading and buttons,
   a `tablist`, a `dialog role="alertdialog"`) and each host draws its own control: on
   iOS the navigation bar, tab bar, system alert.
3. **Leave the rest unsaid:** controls, bars, the type ramp, control colours and dark
   mode are the platform's. A colour or size names a role (`-exact-secondary-label`,
   `font="-exact-footnote"`, `-exact-system-red`); a lookalike of a system control is a
   bug (`bun <exact2>/scripts/no-tells.mjs .` lists literals). The one colour you write
   is the accent, on the root: any system colour (`accent-color="-exact-system-teal"`) or
   one `light-dark(light, dark)` pair, which controls, tabs, progress, links and
   `exact:content` follow. Others are unsaid or roles, since a hex stays put in dark
   mode. A card on a plain page is `-exact-secondary-background`, on a
   `-exact-grouped-background` page `-exact-secondary-grouped-background`; a surface
   inside a card is the tertiary role.
4. **Design your content:** the screen's purpose gets a focal point (big type, a fill,
   a shape), rows a hierarchy, an empty state what to do, a wide window two columns;
   inset-grouped is for settings and forms. `exact:content` draws these
   ([content-design](recipes/content-design.md)).

## The workflow

```sh
bun <exact2>/scripts/exact.mjs new ~/groceries && cd ~/groceries
bun exact.mjs pattern      # lists the starting apps; `pattern list-detail` writes one
bun exact.mjs ios --prebuild  # the first iOS build: minutes, in the background
```

Start from the nearest pattern: it compiles and passes its tests.

| `bun exact.mjs …` | What it does |
| --- | --- |
| `contract build app.contract --json` | `[]` or each diagnostic with its fix; it rewrites the types `app.ts` imports. Builds print the warnings. |
| `verify`, `verify ios` | Builds if stale, runs `app.test.contract` (`test web` alone), then `check` light and dark at phone and desktop widths, in one report. A clean `verify` is done. |
| `agent web --storage s1 "clock data" "tap add" "check both"` | Drives the app as a person would: another screen, a write; a screenshot at milestones. iOS with its bars: [ios-driving](recipes/ios-driving.md). |
| `web`, `ios --run` | The dev loop (it prints its URL, rebuilds on save); a simulator. |

Run `verify` after each edit, `verify ios` at milestones. Drive by `testId`; keep
scratch files in `.exact/` (any other new file is a source, and makes the build stale).

## One screen: the view, its data, its test

`app.contract` is the view, `app.ts` its data module, `app.test.contract` its tests.
`one-screen` holds the screen: a route whose `header` is the iOS navigation bar, over
the `scroll` its `navigationScroll` names. This list takes the scroll's place:

```contract
shape Item
  id: string
  title: string
  done: bool

shape Reply
  ok: bool
  error: string

fn leftText(n: number): string = n == 1 ? "1 item left" : `${n} items left`

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
            input appearance="none" value=draft input=edit submit=add placeholder="Add an item"
              aria-label="New item" enterkeyhint="done" testId="draft"
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

- **Syntax.** Indent with spaces; attributes continue on deeper lines. Quote units
  (`width="50%"`); a bare number is pixels. Subtract with spaces (`a - b`): names hold
  hyphens. A view chooses with `when`/`else`, never `if`; no `form` (a field's Enter is
  its `submit`).
- **Expressions** are free functions (`length(xs)`, `trim(s)`, `map`, `first`); `match`
  takes an option apart; no truthiness. A record copy is `Shape(old, field=value)`.
  Events pass captured arguments, then the payload (`toggle(i.id)` gets the bool).
- **Data.** A `resource` is a reactive read (`pending(x)`, `failed(x)`); a `mutation` a
  write, which `queue` keeps in order, `refreshes` follows with a read and `then` with
  an action. Only the root declares them; children take `props`.
- **After a write**, `then` clears the field only on an ok reply; a refusal keeps the
  text and says what to do, in red under it ([after-a-write](recipes/after-a-write.md)).
  A form of more fields is a sheet.

**The data module** answers each source with exactly its declared shape (an extra field
fails it), reaches only what its `grants` name, and answers a failed fetch as data
([fetch-loading-error](recipes/fetch-loading-error.md)). The clock and randomness are
refused there; time comes as an argument.

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

`clock data` lands replies the next `expect` reads; `clock settle` ends transitions;
`reload` relaunches on the same storage. Every step, and the real-clock drive to end
with: [tests](recipes/tests.md).

## Pitfalls that cost the most

1. **Styling a platform control.** Paint (`background-color`, `border`, `font-size`,
   `height`) on a `button`, field or bar still makes it your box. Emphasis is a part
   (`RoundButton`, `AnswerTile`, `SummaryCard`: `content-design`) or
   `buttonStyle="bordered-prominent"`.
2. **`width="100%"` with padding overflows** (`content-box`, as in CSS). Drop the width;
   give a `flex=1` field `min-width=0`.
3. **A `header` holds a heading, one text after it (the bar's subtitle) and buttons**
   (or a search field, a `tablist`); anything else isn't in the iOS bar.
4. **A drive without `--storage <name>`** refuses storage, so the app looks empty.
5. **Trusting green tests.** They pass on the virtual clock: drive each write once on
   the real clock. `verify` checks the first screen; `"check both"` in a drive, others.

## Look it up

**A pattern** replaces the recipe beside it: read one or the other. A list, its pushed
detail, a create sheet, a delete alert: `pattern list-detail` (in an app you
have, `navigation-stack`). Tabs: `pattern tabs` (`tabs`). A summary, bars, a ring:
`pattern dashboard` (`content-design`). Your own server: `pattern rest` (`rest-backend`).

**A recipe**, [`recipes/<name>.md`](recipes/): `grouped-form`, `after-a-write`,
`keyboard-and-focus`, `number-field` (`inputmode="decimal"`), `numeric-readout`,
`money` (`formatNumber`), `dates-and-days`, `timers`, `timer-that-survives-relaunch`,
`random-and-shuffle`, `symbols`, `chat-thread`, `persisted-setting` (`persist`),
`files-and-grants` (SQLite too), `fetch-loading-error`, `responsive-columns`,
`web-look` (wide screens), `tests`, `ios-driving`, `functions`.

**Otherwise:** `bun exact.mjs contract vocab <name> <name> …` for any tag, property,
function or part; [contract-for-agents](contract-for-agents.md) and
[agent-pitfalls](agent-pitfalls.md) by section; [reference](reference.md) for the data
module's globals and native hatches; `<exact2>/apps/shelf`, a whole app with tests.
