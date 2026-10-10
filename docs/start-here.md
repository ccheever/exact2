# Start here: building an app with Exact

This page is the only required reading for building an app (LLP 1115 D7). The
other guides are for lookup: open them at the section the table at the end
names, or grep them. Don't read them front to back. `contract vocab <name>`
answers "is this tag, attribute or property accepted, and how", and
[`apps/shelf`](../apps/shelf/app.contract) is a whole iPhone app, with tests,
built from the patterns below.

## The idea in five lines

1. **Write the web.** Layout is CSS (flex, gap, padding, `position`) and names are
   HTML's and CSS's (`text-overflow`, `line-clamp`, `role`, `aria-*`), so the web is
   the oracle for how a box lays out.
2. **Ship the platform.** Say what a thing *is* (a `header` with a heading and
   buttons, a `tablist`, a `dialog role="alertdialog"`) and each host draws its own
   control. On iOS that's the navigation bar, the tab bar and the system alert.
3. **Leave colours, fonts and metrics unsaid.** What you leave out is the
   platform's, so dark mode, Dynamic Type and the tint come with it. What you write
   wins, and costs you the platform's look.
4. **When you have to say something, name a role**, never a value:
   `color="-exact-secondary-label"`, `font="-exact-footnote"`, `role="heading"
   aria-level=1`, `AccentColor`. `bun <exact2>/scripts/no-tells.mjs .` lists every
   literal colour, font size and weight left. The roles: `CanvasText`, `Canvas`,
   `AccentColor`, `LinkText`, `GrayText`; `-exact-label`, `-exact-secondary-label`,
   `-exact-tertiary-label`, `-exact-placeholder`, `-exact-separator`,
   `-exact-background`, `-exact-secondary-background`, `-exact-grouped-background`,
   `-exact-secondary-grouped-background`, `-exact-fill`, and the hues
   `-exact-system-red` … `-blue`, `-green`, `-orange`, `-gray` (the full list:
   contract-for-agents.md, "Colours: say a role"). Text styles for `font`/`font-size`:
   `-exact-large-title`, `-exact-title1`…`3`, `-exact-headline`, `-exact-body`,
   `-exact-callout`, `-exact-subheadline`, `-exact-footnote`, `-exact-caption1`/`2`.
5. **A hand-built lookalike of a system control is a bug**: a painted switch, a
   row of buttons drawn as a tab bar, a title in a big `font-size`.

## The workflow

```sh
bun <exact2>/scripts/exact.mjs setup --check   # once per machine: names anything missing (`setup` installs it)
bun <exact2>/scripts/exact.mjs new ~/notes && cd ~/notes
bun exact.mjs ios          # run it in the background now: the first iOS build takes minutes
bun exact.mjs contract types app.contract -o app.contract.d.ts   # the first contract call also compiles the CLI
```

Then loop. Every command runs from the app's directory:

| Command | What it does |
| --- | --- |
| `bun exact.mjs contract build app.contract --json` | `[]`, or every diagnostic with its id and range. Run it after every edit. |
| `bun exact.mjs contract types app.contract -o app.contract.d.ts` | Regenerates the types `app.ts` imports. Rerun it after you change a source's arguments or shape. |
| `bun exact.mjs contract vocab <name>` | Whether a tag, attribute or CSS property is accepted, and its values. |
| `bun exact.mjs test web` | Builds the web app (about a second), then runs `app.test.contract`. |
| `bun exact.mjs agent web --storage s1 "clock data" tree "tap add-note" "screenshot .exact/a.png"` | Drives the app the way a person would. Look at every screenshot. |
| `bun exact.mjs web` | The dev loop at http://127.0.0.1:8765/. It rebuilds on save. |
| `bun exact.mjs ios` / `ios --run` | Builds for a simulator (and launches it). Later builds take a minute or two. |
| `bun exact.mjs test ios` | The same tests on the simulator, after `bun exact.mjs ios`. |
| `bun exact.mjs agent ios --storage s1 --chrome platform "tap tab-feed" "clock +300 real" "screenshot .exact/s.png window"` | Shows the app as a person sees it, with UIKit's bars. |

- On a machine with several booted simulators, set `EXACT_SIM=<udid>` for `ios`,
  `test ios` and `agent ios`. A choice among several is refused, not guessed.
- Under the agent, iOS paints your authored `header` and `tablist` in place of
  UIKit's bars, and they take the same taps. Add `--chrome platform` to see and
  tap the real bars, and use `screenshot <file> window` to capture them. The bars
  animate in real time, so put `clock +300 real` before that screenshot, or it
  shows the old selected tab.
- Keep scratch files, logs and drive scripts in `.exact/`. Any other new file in
  the app folder counts as a source, so the driver treats the build as stale.
- Drive by `testId`, never by screen coordinates. Give every control a `testId`.

## The files

| File | What it holds |
| --- | --- |
| `app.contract` | The view: shapes, the root component (state, resources, actions, view), routes. |
| `app.ts` | The data module: every source the Contract calls (storage, `fetch`, sorting, domain logic) and its `grants`. A Rust data crate is the alternative ([reference](reference.md#rust-data-sources)). |
| `app.test.contract` | Authored tests. The same file runs on the web and iOS. |
| `app.json` | The manifest: name, bundle id, hosts. Its `$schema` gives an editor every key. |

Contract holds no I/O. Anything that touches a file, the network or the clock's
date is a source in `app.ts` that the Contract calls by name.

## Contract in one pass

This complete program compiles. Each line is a pattern you'll use:

```contract
shape Task
  id: string
  title: string
  done: bool

shape Ack
  ok: bool

fn countText(n: number): string = n == 1 ? "1 task" : `${n} tasks`

component Tasks
  state draft = ""
  state showDone = true
  resource tasks = loadTasks() as shape list<Task> else empty()
  mutation saved as shape Ack queue refreshes tasks
  derive shown = filter(tasks, t => showDone or not t.done)
  derive left = length(filter(tasks, t => not t.done))

  action edit(v: string)
    draft = v
  action add
    let title = trim(draft)
    if title != ""
      send saved = addTask(title)
      draft = ""
  action toggle(id: string, done: bool)
    send saved = setDone(id, done)
  action flipShowDone
    showDone = not showDone

  view
    column gap=8 padding=16
      row gap=8
        input value=draft input=edit submit=add placeholder="New task" aria-label="New task" testId="draft" flex=1 min-width=0
        button press=add disabled=(trim(draft) == "") testId="add"
          text "Add"
      each t in shown key=t.id
        row gap=8 align-items="center" testId=`task-${t.id}`
          input type="checkbox" checked=t.done change=toggle(t.id) aria-label=t.title testId=`done-${t.id}`
          text t.title flex=1
      when length(tasks) == 0
        text "Nothing yet." color="-exact-secondary-label" testId="empty"
      else
        text countText(left) testId="left"
      button press=flipShowDone testId="show-done"
        text (showDone ? "Hide Done" : "Show Done")
```

- **Layout of the source.** Top-level declarations start in column 1. Indent
  with spaces, and comment with `//`. An element's attributes can continue on
  deeper lines that each start with `name=`. Use double quotes for strings and
  backticks for templates. Quote units (`width="50%"`); a bare number is pixels.
  Names can contain hyphens, so subtraction between names is `a - b` with spaces.
- **Elements.** Use `column`, `row` and `view` (block) for `div`, and `text` for
  `span`/`p`. A heading is `text role="heading" aria-level=N`, and an image is
  `image "symbol:add"`, an `assets/` path or a URL. Also: `button`, `input`,
  `textarea`, `select`/`option`, `scroll`, `list`/`section`, `header`/`footer`,
  `dialog`, `progress` and `link`. There's no `form`: a field's Enter is its
  `submit`.
- **Expressions** are free functions, not methods: `length(xs)`, `trim(s)`,
  `includes(s, q)`, `toLowerCase(s)`, `filter(xs, x => …)`, `map`, `first` and
  `at` (both return options), `concat(xs, [x])` and `join`. Use
  `match parseNumber(s) { case some(n) => …, case none => … }` to read a number.
  There's no truthiness, so write `s != ""`. A record is
  `Shape(field=value, …)`, and a copy is `Shape(old, field=value)`.
- **State and actions.** An action's reads see the state as it was when the
  action started, and all its writes commit together. Use `let` for a value you
  compute and reuse. Actions can call other actions; a view chooses with
  `when`/`else when`/`else`, never `if`. `each x in xs key=x.id` needs a stable
  key.
- **Events** bind actions, with arguments you capture first and the event's
  payload appended: `press=open(n.id)`, `input=edit` (gets the text), and
  `change=toggle(t.id)` on a checkbox (gets the `bool`).
- **Styles.** Write CSS properties as attributes (`padding="8px 16px"`, `gap=8`,
  `flex=1`, `min-height=0`). A `style Name` block holds literal attributes for
  `class=Name`, and node attributes win over it. There's no cascade.
- **Resources** are reactive reads: `resource x = source(args) as shape T else
  empty(field=constant)`. They're asked again when an argument changes. Use
  `pending(x)` for a spinner and `failed(x)`/`failure(x)` for a request that
  failed. Only the root component declares resources, mutations and tasks;
  children get values and actions as props.
- **Mutations** are writes: `send saved = addTask(title)`. Their reply reads as
  `option<T>`. `refreshes tasks` reads the resource again after the reply lands.
  `queue` runs sends in order, one at a time, and lets one action send twice.
  `then afterSave` runs an action after the reply.
- **Components.** The first component in the file is the root. A child declares
  `props` and is used as `Row(item=x, open=open)`; every prop is required. A
  child's `state` can start from a prop. That's how a form edits a saved record:
  render the child once the record has loaded.

### Refusals you'll meet

Each diagnostic names its fix. These come up most:

| Diagnostic | Meaning and fix |
| --- | --- |
| `syntax-placeholder-call` | A resource's placeholder is `else empty()` (`empty(field=…)` for a record), never `else []`. |
| `syntax-stray-keyword` (`if` in a view) | A view chooses with `when`; `if` is a statement inside actions. |
| `type-refused-idiom` | You wrote a JavaScript spelling (`.length`, `count`, `push`). Use what it names (`length(xs)`, `concat`). |
| `lower-zero-size` | A `button` (or box) with no children and no size can't be pressed. Give it a `text` or an `image`. |
| `lower-root-region` | A view's root is one element, never a `when`/`each`. |
| `route-template` | Build locations with `path("note", id)`, never a template string. |
| `lower-route-place` | A route (a node with `navigationKey`) must be a direct child of the root or of a `role="tabpanel"`, through `each`/`when` only. |
| `lower-route-scroll` | `navigationScroll` must name a scroller in that route. |
| `lower-alertdialog` | An `alertdialog` holds only text, buttons that close it and one Cancel. Use `role="dialog" aria-modal=true` for anything you lay out yourself. |
| `analyze-send-twice` | One action sends one mutation twice. Declare it `queue`, or send one combined request. |
| `analyze-call-stale-read` | A called action reads a slot its caller just wrote, and would see the old value. Pass the value as an argument instead. |
| `type-initializer-scope` | A `state` starts from props or earlier states only. Use a `derive`, or a child component made once the data is in. |
| `type-now-renamed` | There's no `now()`. Use `performanceNow()` for durations, or `time.epochAtZero + performanceNow()` with `resource time = exactTime() as shape Time` for the date. |

## The app skeleton: tabs, stacks and an alert

Copy this into `app.contract` and grow it. It's [Shelf](../apps/shelf/app.contract)'s
structure (lines 27–120 and 181–465). The routes table nests a tab's pushed
screens under it:

<!-- check: app -->
```contract
routes nav
  tab notes "/"
    note "/note/:id"
    add "/add"
  tab feed "/feed"

shape Note
  id: string
  title: string
  createdAt: number

shape Notes
  ready: bool
  notes: list<Note>

shape Ack
  ok: bool

shape Time
  epochAtZero: number

shape Post
  id: string
  title: string

shape Feed
  ok: bool
  error: string
  posts: list<Post>
```

The root holds every resource, the mutation and the actions. `nav` is the
router's own state; assign `select`, `push`, `back` or `go` to it:

<!-- check: app -->
```contract
component App
  state query = ""
  state filter = "all"
  state aimed = ""
  state draftTitle = ""
  state submitted = false

  resource time = exactTime() as shape Time
  resource lib = loadNotes() as shape Notes else empty(ready=false)
  resource feed = loadFeed() as shape Feed else empty(ok=true)
  mutation changed as shape Ack queue refreshes lib

  derive current = top(nav)
  derive shown = filter(lib.notes, n => includes(toLowerCase(n.title), toLowerCase(trim(query))))
  derive titleError = trim(draftTitle) == "" ? "Enter a title." : ""

  action pick(name: string)
    nav = select(nav, name)
  action follow(url: string)
    nav = go(nav, url)
  action back
    nav = back(nav)
  action open(id: string)
    nav = push(nav, path("note", id))
  action openAdd
    draftTitle = ""
    submitted = false
    nav = push(nav, "/add")
  action search(v: string)
    query = v
  action show(f: string)
    filter = f
  action editTitle(v: string)
    draftTitle = v
  action save
    submitted = true
    if titleError == ""
      send changed = addNote(trim(draftTitle), time.epochAtZero + performanceNow())
      nav = back(nav)
  action aim(id: string)
    aimed = id
  action askRemove(id: string)
    aimed = id
    showModal("remove")
  action remove
    if current.name == "note"
      nav = back(nav)
    send changed = removeNote(aimed)
  action reload
    refresh feed
```

The view: one panel per tab, each a stack of routes, then the tab bar, then the
alert. Each `when e.name == …` holds one screen; the next section fills them in.

<!-- check: app -->
```contract
  view
    main navigationKey=`${current.id}` navigationBack="back" navigate=follow testId="app"
      width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 min-height=0 position="relative"
        each t in nav.tabs key = t.name
          column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
            each e in t.stack key = e.id
              when e.name == "notes"
                column navigationKey=`${e.id}` position="absolute" inset=0
                  // "A large title with a bar button, search and a segmented control"
              when e.name == "note"
                column navigationKey=`${e.id}` position="absolute" inset=0
                  // "A pushed screen: a grouped list"
              when e.name == "add"
                column navigationKey=`${e.id}` position="absolute" inset=0
                  // "A sheet with Cancel and Save, and validation"
              when e.name == "feed"
                column navigationKey=`${e.id}` position="absolute" inset=0
                  // "Fetch with loading, error, retry and pull to refresh"
      row role="tablist" testId="tabs"
        button role="tab" aria-controls="panel-notes" aria-selected=(nav.tab == "notes") press=pick("notes") testId="tab-notes" flex=1
          image "symbol:document"
          text "Notes"
        button role="tab" aria-controls="panel-feed" aria-selected=(nav.tab == "feed") press=pick("feed") testId="tab-feed" flex=1
          image "symbol:sparkles"
          text "Feed"
      dialog id="remove" role="alertdialog" aria-label="Delete Note?"
        text "This can’t be undone."
        button press=remove destructive=true commandfor="remove" command="close" testId="remove-confirm"
          text "Delete"
        button commandfor="remove" command="close" testId="remove-cancel"
          text "Cancel"
```

- **Tabs.** The root `tablist` is the tab bar: each tab is a symbol over a
  label, names its panel with `aria-controls`, and calls `select`. Every tab's
  stack stays mounted. To hide the bar on a screen, set `display="none"` on the
  tablist; never remove it with `when`. Symbol roles include `add`, `delete`,
  `compose`, `search`, `more`, `settings`, `home`, `person`, `people`,
  `document`, `book`, `bookmark`, `calendar`, `chart`, `sparkles`, `heart`,
  `star`, `photo`, `camera`, `share`, `info`, `lock`, `checkmark`, `close`,
  `forward-chevron`, `minus`, `pencil` and `notifications` (`-fill` variants for
  some). An unknown role is refused with the full list.
- **Routes.** Each route is `position="absolute" inset=0`, a direct child of the
  root or a tabpanel. A pushed route gets the system Back button and edge swipe,
  and a sheet swipes down, with nothing written for either. `navigationBack="back"`
  names the control (by `id`) that those gestures press, and `navigate=follow`
  takes the browser's or the system's location. Covered routes stay mounted, so
  two screens must not share a `testId`.
- **The alert.** A `dialog role="alertdialog"` is `UIAlertController`:
  `aria-label` is the title, the `text` is the message, each action button
  closes it (`commandfor`/`command="close"`), and the Cancel is a closing button
  with no `press`. Open it from any action with `showModal("remove")`, or from a
  button with `commandfor="remove" command="show-modal"`. Shelf: lines 129–141
  and 453–464.

## Native patterns

Each snippet below fills a `when e.name == …` route in the skeleton. All of
them compile in it.

### A large title with a bar button, search and a segmented control

The route's first child is its `header`, which becomes the navigation bar on
iOS. A level-1 heading is the large title, its buttons are the bar items, and an
`input type="search"` is the bar's search field. A scroller right after the
header, named by `navigationScroll`, is what scrolls and collapses the title. A
route doesn't scroll by itself. A `tablist` in the content is a segmented
control. (Shelf 194–265.)

<!-- check: route notes -->
```contract
column navigationKey=`${e.id}` navigationScroll="notes-list" testId="route-notes"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" flex-wrap="wrap" align-items="center" gap=8 padding="8px 16px"
    text "Notes" role="heading" aria-level=1 flex=1
    button press=openAdd aria-label="Add Note" testId="add-note"
      image "symbol:add"
    input type="search" value=query input=search placeholder="Search" aria-label="Search notes" testId="search" width="100%"
  scroll id="notes-list" flex=1 min-height=0
    row role="tablist" aria-label="Filter" margin="8px 16px"
      button role="tab" aria-selected=(filter == "all") press=show("all") testId="filter-all" flex=1
        text "All"
      button role="tab" aria-selected=(filter == "recent") press=show("recent") testId="filter-recent" flex=1
        text "Recent"
    each n in shown key = n.id
      NoteRow(note=n, open=open, aim=aim, remove=askRemove)
  column id="note-menu" popover="auto" role="menu" aria-label="Note"
    button press=askRemove(aimed) destructive=true popovertarget="note-menu" popovertargetaction="hide" testId="menu-remove"
      image "symbol:delete"
      text "Delete…"
```

The `note-menu` popover at the end is the long-press menu that every row names.

### A row: tap to push, long-press menu, swipe to delete

`contextPopover` names the menu, and the row's own `contextmenu` action runs
first, so one menu serves every row. The swipe is a horizontal snap scroll that
names its content and its trailing action, and on iOS the action is UIKit's own.
A symbol-only button needs an `aria-label`. (Shelf 466–524.)

<!-- check: file -->
```contract
component NoteRow
  props
    note: Note
    open: action
    aim: action
    remove: action
  view
    scroll swipeContent=`note-${note.id}` swipeTrailing=`delete-${note.id}`
      width="100%" overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
      row width="100%"
        button id=`note-${note.id}` press=open(note.id) contextmenu=aim(note.id) contextPopover="note-menu" testId=`note-${note.id}`
          width="100%" flex-shrink=0 scroll-snap-align="start" text-align="start" padding="12px 16px"
          text note.title
        button id=`delete-${note.id}` destructive=true press=remove(note.id) aria-label="Delete" testId=`delete-${note.id}`
          flex-shrink=0 scroll-snap-align="start"
          image "symbol:delete"
```

### A pushed screen: a grouped list

A level-2 heading is the inline title. `list appearance="auto"
listStyle="inset-grouped"` of `section`s (each an optional `header`, its rows
and an optional `footer`) draws as Settings does. A row of two `text`s is a
title and value. (Shelf 266–291 and 526–603.)

<!-- check: route note -->
```contract
column navigationKey=`${e.id}` navigationScroll="note-list" testId="route-note"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" align-items="center" padding="8px 16px"
    text "Note" role="heading" aria-level=2
  list id="note-list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    each n in filter(lib.notes, x => x.id == e.params.id) key = n.id
      section
        header
          text "Title"
        row
          text n.title testId="detail-title"
      section
        button press=askRemove(n.id) destructive=true testId="delete-note"
          text "Delete Note"
```

Route parameters are strings: `e.params.id`.

### A sheet with Cancel and Save, and validation

`navigationPresentation="modal"` makes the route a sheet. Cancel is the control
whose `id` is the root's `navigationBack`, so the swipe down presses it too. A
field in a grouped row takes `appearance="none"`, or it draws its own rounded
box inside the card. Show errors only after the first Save (`submitted`). A
footer's text is always the system's grey, so don't colour it. (Shelf 292–361.)

<!-- check: route add -->
```contract
column navigationKey=`${e.id}` navigationPresentation="modal" navigationScroll="add-form" testId="route-add"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
    button id="back" press=back testId="add-cancel"
      text "Cancel"
    text "New Note" role="heading"
    button press=save testId="add-save"
      text "Save"
  list id="add-form" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
    section
      header
        text "Title"
      row
        input appearance="none" value=draftTitle input=editTitle placeholder="Required" aria-label="Title" autofocus=true testId="add-title" flex=1
      footer
        text (submitted ? titleError : "") testId="error-title"
```

A field writes raw text to state on every `input`. Validate a derive of it, and
normalize only on `change` (Enter or blur), never as the person types. Other
controls: `input type="checkbox" switch` is a switch, `type="range"` a slider,
`type="date"`/`"time"` a date picker, and `select` of `option`s a pop-up menu.

### Fetch with loading, error, retry and pull to refresh

`refresh=` on the scroller is pull to refresh, and `refreshing` holds the
spinner until the reload lands. `progress` with no `value` is the activity
indicator. The error is data the source answers (`ok: false`), so the view
branches on it. (Shelf 362–409 and `app.ts` 170–190.)

<!-- check: route feed -->
```contract
column navigationKey=`${e.id}` navigationScroll="feed-list" testId="route-feed"
  position="absolute" inset=0 display="flex" flex-direction="column"
  header display="flex" align-items="center" padding="8px 16px"
    text "Feed" role="heading" aria-level=1
  scroll id="feed-list" refresh=reload refreshing=(pending(feed) and length(feed.posts) > 0) flex=1 min-height=0
    when pending(feed) and length(feed.posts) == 0
      row justify-content="center" padding=40
        progress aria-label="Loading" testId="feed-loading"
    else when not feed.ok
      column align-items="center" gap=8 padding=32 testId="feed-error"
        text "Couldn’t Load" role="heading" aria-level=3
        text feed.error color="-exact-secondary-label"
        button press=reload testId="retry"
          text "Try Again"
    else
      each p in feed.posts key = p.id
        text p.title padding="10px 16px" testId=`post-${p.id}`
```

### Persistence and fetch: the data module

`app.ts` answers every source the Contract calls, by name, with exactly the
declared shape: an extra field fails the resource. A data module can't read the
clock or randomness (`Date.now()`, `new Date()`, `Math.random()`, timers): the
build refuses them. Time comes in as an argument from the Contract
(`time.epochAtZero + performanceNow()`, as `save` sends above), and
`crypto.randomUUID()` works. Grant what the module touches, one line each. A
file directly in `app:/data` needs no `mkdir`. (Shelf's `app.ts`, with a seed
and a serial write queue.)

```ts
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.notes'; // exact new writes this line
export const grants = [
  'fs.read app:/data/notes.json',
  'fs.write app:/data/notes.json',
  'net.fetch https://jsonplaceholder.typicode.com',
].join('\n');

type Note = Result<'loadNotes'>['notes'][number];
const FILE = 'app:/data/notes.json';
let notes: Note[] | null = null; // memory; the file is what survives a launch

async function load(storage: Storage): Promise<Note[]> {
  if (notes) return notes;
  try {
    const saved = JSON.parse(new TextDecoder().decode(await storage.fs.readFile(FILE))) as Note[];
    notes = saved.map((n) => ({ id: String(n.id), title: String(n.title), createdAt: Number(n.createdAt) }));
  } catch (e) {
    // 'bake' (the build has no storage) and 'agent' (a drive without --storage) pass through
    if ((e as { code?: string }).code !== 'ENOENT') throw e;
    notes = []; // first launch
  }
  return notes;
}

async function save(storage: Storage, next: Note[]): Promise<Result<'addNote'>> {
  notes = next;
  await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify(next)));
  return { ok: true };
}

const sources: Sources = {
  loadNotes: async (_args, _store, storage) => ({ ready: true, notes: await load(storage) }),
  addNote: async ([title, at], _store, storage) =>
    save(storage, [{ id: crypto.randomUUID(), title, createdAt: at }, ...(await load(storage))]),
  removeNote: async ([id], _store, storage) => save(storage, (await load(storage)).filter((n) => n.id !== id)),
  loadFeed: async () => {
    try {
      const res = await fetch('https://jsonplaceholder.typicode.com/posts?_limit=10');
      if (!res.ok) throw new Error(`The server answered ${res.status}.`);
      const posts = (await res.json()) as Array<{ id: number; title: string }>;
      return { ok: true, error: '', posts: posts.map((p) => ({ id: String(p.id), title: p.title })) };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e; // no network at build: stay unbaked
      return { ok: false, error: e instanceof Error ? e.message : 'Something went wrong.', posts: [] };
    }
  },
};

export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
```

- A write is a `mutation … queue refreshes lib`: sends run one at a time, in
  order, and the list is read again when each reply lands.
- To keep a secret (a token), grant `secret.keep <name>` and use
  `store.set`/`store.get`, never a file. For SQLite, see
  [the human guide's data module](contract-for-humans.md#writing-the-data-module).
- `console.log` in a source shows up in the agent's `logs`.

## Tests

`app.test.contract` drives the app by `testId` and runs the same way on the web
and iOS (`bun exact.mjs test web`, `test ios`). Each test starts fresh, with its
own storage, after the app's first data has landed:

```contract-test
test "add validates, saves, persists, and deletes after asking"
  tap "add-note"
  tap "add-save"
  expect text "error-title" == "Enter a title."
  type "add-title" "Milk"
  tap "add-save"
  clock data
  expect state lib.notes.0 .title == "Milk"
  reload
  expect state lib.notes.0 .title == "Milk"
  tap "Milk"
  expect tree has "route-note"
  tap "delete-note"
  clock settle
  tap "remove-confirm"
  clock settle
  clock data
  expect state current.name == "notes"
  expect state lib.notes == []

test "the feed shows an error, then retries"
  fail fetch "https://jsonplaceholder.typicode.com"
  tap "tab-feed"
  expect tree has "feed-error"
  pass fetch "https://jsonplaceholder.typicode.com"
  tap "retry"
  clock data
  expect tree missing "feed-error"
```

- **`clock data`** lands what's in flight (storage, fetch) without moving the
  clock. Put it after any input whose reply the next `expect` reads. The web
  often answers in time without it, but iOS doesn't.
- **`clock settle`** runs the clock to where transitions end. Put it after
  opening or closing a sheet, an alert or a push, and before tapping inside an
  alert. Never wait in real time (`clock +1000 real`) for a transition.
- **`reload`** relaunches the app on the same storage, which is how you show
  persistence. `fail fetch "<prefix>"` and `pass fetch` test the error path with
  the app's real handling.
- **Targets.** `tap "<testId>"`, or a node's exact text or label (`tap "Milk"`);
  `tap "<id>" contextmenu` long-presses. `type "<id>" "text"` sets a field, a
  `select`, a date or a checkbox (`"true"`). Assert with `expect text "<id>" ==
  "…"`, `expect tree has|missing "<id>"` and `expect state <name>.<field> ==
  …`. In a state path, a list index is followed by a space before the field
  (`lib.notes.0 .title`).
- An interactive drive uses the same operations: `bun exact.mjs agent web
  --storage s1 "clock data" "tap add-note" "type add-title Milk" "tap add-save"
  "clock data" state "screenshot .exact/x.png"`. Without `--storage <name>`, a
  drive refuses every storage call, and the app looks empty.

## The pitfalls that cost agents the most

1. **Reading reference docs instead of building.** Compile early and often;
   each diagnostic names its fix. Look things up by section (the table below).
2. **A screen that doesn't scroll.** A route is a box. Its content goes in
   `scroll id=… flex=1 min-height=0` right after the `header`, with
   `navigationScroll` naming that `id`.
3. **Styling the platform's controls.** A `background-color`, `border`,
   `border-radius`, `font-size` or `height` on a `button`, field or bar turns it
   into your own box. Leave them off, and use `buttonStyle="bordered"` or
   `"bordered-prominent"` for emphasis.
4. **`width="100%"` with padding overflows.** Boxes are `content-box`, as in
   CSS. Drop the `width` (a block fills its line), or add
   `box-sizing="border-box"`. Use `min-width=0` on a `flex=1` field in a row.
5. **The clock or randomness in `app.ts`.** Pass time in from the Contract.
6. **An answer outside its shape.** An extra or missing field fails the resource
   (`state` shows it under `failed`). Map backend rows field by field.
7. **Waiting in real time in tests.** Use `clock data` after saves and
   `clock settle` around sheets, alerts and pushes.
8. **A drive with no `--storage`.** Storage is refused and the app looks empty.
   Authored tests get their own store.
9. **Trusting a passing test.** A test passes on a layout that spills off the
   screen. Look at screenshots: web, then iOS with `--chrome platform` and
   `screenshot … window`.
10. **Navigating away in the same action as a save, then reloading.** The write
    lands after the screen changes. In tests, `clock data` before `reload`.
11. **Shared `testId`s across screens.** Covered routes stay mounted, so
    `tree` finds two nodes. Give each screen its own ids.
12. **Starting the iOS build late.** It takes minutes cold. Start it in the
    background as soon as the app exists, and keep working on the web.

## When you need more

Look it up; don't read it through. `grep -n "<word>" <exact2>/docs/*.md
<exact2>/apps/shelf/*` is often fastest. `A` is
[contract-for-agents](contract-for-agents.md), `P` is
[agent-pitfalls](agent-pitfalls.md), `G` is [contract-grammar](contract-grammar.md)
and `R` is [reference](reference.md).

| Topic | Where |
| --- | --- |
| Is a tag, attribute or property accepted, and its values | `bun exact.mjs contract vocab <name>` |
| A full iPhone app with tests | [`apps/shelf`](../apps/shelf/) |
| Every declaration form | A [Language inventory](contract-for-agents.md#language-inventory) |
| Expressions, lists, numbers, money, dates | A [Values, expressions, and functions](contract-for-agents.md#values-expressions-and-functions); G [Standard functions](contract-grammar.md#standard-functions-and-intrinsics) |
| Action semantics, calling actions, editing a field | A [State and action semantics](contract-for-agents.md#state-and-action-semantics) |
| Props, child state, a form that edits a record, `provide`/`inject`, slots | A [Composition and lifetime](contract-for-agents.md#composition-and-lifetime) |
| Resources, mutations, `queue`, `then`, `failure`, optimistic overlay | A [Data requests and side effects](contract-for-agents.md#data-requests-and-side-effects) |
| Native controls and what iOS draws for each | A [Views, layout, and interaction](contract-for-agents.md#views-layout-and-interaction) ("Prefer native controls") |
| Menus, submenus, context menus, alerts, action sheets | A [A confirmation](contract-for-agents.md#a-confirmation) and the paragraphs around it |
| Routes, `path()`, `navigationScroll` | A [Routes and web documents](contract-for-agents.md#routes-and-web-documents) |
| Tabs, stacks, sheets, detents, hiding the tab bar | A [Tabs and stacks](contract-for-agents.md#tabs-and-stacks) |
| Timers (`task`), time, motion, pointer, keys, haptics | A [Time, motion, graphics, and platform facts](contract-for-agents.md#time-motion-graphics-and-platform-facts) |
| Driver operations, every test step, launch lines | A [Inspection and testing](contract-for-agents.md#inspection-and-testing); G [Authored tests](contract-grammar.md#authored-tests) |
| A refusal not listed here | A [Repair common mistakes](contract-for-agents.md#repair-common-mistakes), or grep its id under `contract/` |
| Compiles but lays out wrong | P [Layout](agent-pitfalls.md#layout), [Lists and scrolling](agent-pitfalls.md#lists-and-scrolling) |
| iOS bars, sheets, gestures, back | P [Native presentation and navigation](agent-pitfalls.md#native-presentation-and-navigation-ios) |
| Fields, keys, swipes, drags | P [Input](agent-pitfalls.md#input) |
| A test that is flaky, or differs between web and iOS | P [Driving and testing](agent-pitfalls.md#driving-and-testing) |
| Exact syntax, events and payloads, tags | G [Events](contract-grammar.md#events), [Built-in tags](contract-grammar.md#built-in-tags) |
| Data module globals, storage API and error codes, grants, SQLite | R [What a data module can use](reference.md#what-a-data-module-can-use); [human guide: Writing the data module](contract-for-humans.md#writing-the-data-module) |
| Rust data sources | R [Rust data sources](reference.md#rust-data-sources) |
| Native code for what Contract can't say | R [Access hatches](reference.md#access-hatches) |
| Notifications, Health, sounds, picked images, documents | R, one section each |
