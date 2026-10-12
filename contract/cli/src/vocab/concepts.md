<!--
`contract vocab <name>` answers each concept below with its whole idiom (LLP 1116.004 D5.1):
what a builder looked up a recipe or the grammar for, answered in one lookup. A section is
`## <name>: <what it does>`, then `names: …`, the other names it answers to, then the answer:
a sentence or two, the minimal complete code, and the recipe for depth. A name that is also a
tag, attribute or function prints its entry first, then the concept. Every `contract` block
compiles (`contract/cli/tests/it/vocab.rs`).
-->

## scroll: scrolling to the top, or to an item
names: scrollTo, scrollToTop, scroll-to-top, scrollToIndex, scrollIntoView, scrollTop

From an action, `scrollIntoView(id)` scrolls every scroller above the element with that `id`
until it shows, as the web's `Element.scrollIntoView()` does: `block="start"` by default
(`"center"`, `"end"`, `"nearest"`), `behavior="smooth"` to animate. The top is the first
element's `id`; an item is its own.

```contract
component App
  state rows = ["Apples", "Bread", "Milk", "Tea", "Eggs"]
  action toTop
    scrollIntoView("top", behavior="smooth")
  action toItem(r: string)
    scrollIntoView(`row-${r}`, block="center")
  view
    column height="100%" display="flex" flex-direction="column"
      row gap=8 padding=8
        button press=toTop testId="to-top"
          text "Top"
        button press=toItem("Tea") testId="to-tea"
          text "Tea"
      scroll flex=1 min-height=0
        text "Groceries" id="top" role="heading" aria-level=2 padding=16
        each r in rows key=r
          text r id=`row-${r}` padding=16
```

- A virtualized `list` (rows not built yet) scrolls to a row by its key:
  `scrollIntoView("list-id", key, block="center")`. A chat opens at its newest row with
  `scroll-start="end"` on the `list`, and `scrollFollowEnd=true` keeps it there.
- `scrollTop=y` on a scroller moves it only when `y` changes, so a second "top" does
  nothing; `scroll=moved` reports the offset (`action moved(x: number, y: number)`).

More: docs/contract-grammar.md#host-commands (`scrollIntoView`)

## loading: what shows before the data, and while it loads
names: spinner, skeleton, baked, bake, first-frame

A resource's `else` is its value until the first answer: `else empty()` is its shape's zero
(`false`, `""`, `0`, `[]`), so a `loaded: bool` the source always answers `true` tells the
first frame from an empty answer. `pending(r)` is true while a request is out, `failed(r)`
once one failed with no answer. `progress` with no `value` is the system spinner.

```contract
shape Feed
  loaded: bool
  error: string
  titles: list<string>

component App
  resource feed = loadFeed() as shape Feed else empty()
  action reload
    refresh feed
  view
    column padding=16 gap=8
      when failed(feed) or feed.error != ""
        text "Couldn’t load the feed." testId="feed-error"
        button press=reload testId="retry"
          text "Try Again"
      else when not feed.loaded
        progress aria-label="Loading" testId="feed-loading"
      else when length(feed.titles) == 0
        text "Nothing here yet." color="-exact-secondary-label" testId="feed-empty"
      else
        each t in feed.titles key=t
          text t
```

- The build asks each source once and bakes its answer into the first frame. Storage and
  the network refuse it there with `code: 'bake'`: let that throw (`catch (e) { if (e.code
  === 'bake') throw e; … }`) so the placeholder shows and the source is asked at launch.
  Caught and answered as an error, the error is baked into every launch's first frame.
- A refresh keeps the last answer on screen (`pending(feed)` is true meanwhile): show the
  spinner only while nothing is loaded.

More: docs/recipes/fetch-loading-error.md (retry, pull to refresh, the data module)

## plural: a count with its noun ("1 card", "2 cards")
names: plurals, pluralize, count-noun

Choose the noun by the number: `` `${n} cards` `` reads "1 cards". A `fn` keeps the choice in
one place; `formatNumber(n, "decimal")` groups thousands.

```contract
fn cardsText(n: number): string = n == 0 ? "No cards" : n == 1 ? "1 card" : `${formatNumber(n, "decimal")} cards`

component App
  state due = 1
  state spots = 1
  view
    column padding=16 gap=8
      text cardsText(due) testId="due"
      text `${spots} ${spots == 1 ? "spot" : "spots"} left` testId="spots"
```

Contract has no `Intl.PluralRules`: a language with more forms gets its strings from the
data module.

## choice: one value from two to five (a preset, units, a period)
names: radiogroup, radio, segmented, segmented-control, segmentedcontrol, UISegmentedControl

A `row role="radiogroup"` of `button role="radio" aria-checked=…`, each pressing its value, is
the platform's segmented control: UIKit's `UISegmentedControl`, AppKit's segmented control,
and on the web a segmented radio group (one Tab stop, the arrow keys move the choice). A
`tablist` switches panels; a value is a `radiogroup`.

```contract
component App
  state tip = 18
  action pick(p: number)
    tip = p
  view
    column padding=16 gap=8
      row role="radiogroup" aria-label="Tip"
        each p in [15, 18, 20, 25] key=p
          button role="radio" aria-checked=(tip == p) press=pick(p) testId=`tip-${p}` flex=1
            text `${p}%`
      text `Tip: ${tip}%` testId="tip"
```

- Each option is one short `text` (or a symbol `image` with an `aria-label`); leave its
  look unsaid. Six or more options, or long labels: a `select`. On or off: `input
  type="checkbox" switch`.
- `state tip = 18 persist` keeps the choice across launches.

More: docs/recipes/persisted-setting.md

## screen: a second screen, a list and its detail, and Back
names: navigate, navigation, back, detail, route, routes, router, list-detail, master-detail, deep-link, react-router

A second screen is a route. `routes` gives each screen a path, the root draws `stack(nav)`, a
row `push`es its detail and `back` returns. That gives the web its Back (the browser's Back
pops the stack; Forward comes through `navigate`), a reload that keeps the place and a link to
every screen (`/recipe/Ramen` opens on that recipe, the list under it), and iOS its navigation
bar, Back button and edge swipe. Screens swapped by `when showingDetail` get none of these
(`lower-screen-by-state`).

```contract
routes nav
  recipes "/"
    recipe "/recipe/:name"

component App
  action open(name: string)
    nav = push(nav, path("recipe", name))
  action back
    nav = back(nav)
  action follow(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
      each e in stack(nav) key=e.id
        when e.name == "recipes"
          column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0 display="flex" flex-direction="column"
            header padding="8px 16px"
              text "Recipes" role="heading" aria-level=1
            list id="list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
              section
                each n in ["Pancakes", "Ramen", "Tacos"] key=n
                  button press=open(n) testId=`recipe-${n}`
                    text n
                    image "symbol:forward-chevron"
        when e.name == "recipe"
          column navigationKey=`${e.id}` position="absolute" inset=0
            header padding="8px 16px"
              text e.params.name role="heading" aria-level=2
            text "Two eggs, a cup of flour, a cup of milk." padding=16 testId="recipe-body"
```

- A window 900 px wide or more on the web shows the list beside its detail, as a split view
  does, with nothing to write: the first route's rows push (a `link`, or a `press` that writes the
  router) and draw the `forward-chevron`; a chevron on a row that only selects is decoration. A
  phone pushes. A row pressed while a detail shows pushes another; to swap the detail instead,
  `open` replaces when one shows: `nav = top(nav).name == "recipe" ? replace(nav,
  path("recipe", name)) : push(nav, path("recipe", name))`.
- Each route keeps its own scroll: a pushed screen opens at its top, and Back returns to the
  list where it was left. Build locations with `path("recipe", name)`; a route's parameters
  are strings (`e.params.name`). In tests, `clock settle` after each push and back. A form
  over the screen is a sheet (`contract vocab modal`).

More: docs/recipes/navigation-stack.md (a list, its detail, a create sheet, a delete alert)

## modal: a form over the screen, with Save and Cancel
names: dialog, sheet, closedby, popup

A form you lay out (fields, Save, Cancel) is a route presented as a sheet:
`navigationPresentation="modal"` on its root, its Cancel the control whose `id` the root's
`navigationBack` names, so the iOS swipe down and the web's Escape press it, and the screen
under it is inert. A question with buttons only is a `dialog role="alertdialog"`
(`contract vocab confirm`). A `dialog` you lay out yourself is a smaller thing: the web's and
macOS's modal `dialog`, and on iOS a sheet as tall as its content, with no bar.
`role="dialog"` on a `column` gets no Escape, focus or sheet anywhere.

```contract
routes nav
  home "/"
    rename "/rename"

component App
  state name = "Ada"
  state draft = ""
  action open
    draft = name
    nav = push(nav, "/rename")
  action back
    nav = back(nav)
  action edit(v: string)
    draft = v
  action save
    name = trim(draft)
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      each e in stack(nav) key=e.id
        when e.name == "home"
          column navigationKey=`${e.id}` position="absolute" inset=0 padding=16 gap=8
            text name testId="name"
            button press=open testId="rename"
              text "Rename"
        when e.name == "rename"
          column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
            header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
              button id="back" press=back testId="cancel"
                text "Cancel"
              text "Rename" role="heading"
              button press=save disabled=(trim(draft) == "") testId="save"
                text "Save"
            input value=draft input=edit aria-label="Name" autofocus=true margin=16 testId="field"
```

- In tests, `clock settle` after the push and after Save. A sheet's height:
  `navigationDetent="medium"` (or `"fit-content"`).

More: docs/recipes/navigation-stack.md (a list, its detail, a create sheet, a delete alert)

## confirm: ask before deleting
names: confirmation, alert, alertdialog, delete-confirmation, Alert, showModal

A `dialog role="alertdialog"`: its `aria-label` the question, a `text` the message, each action
a `button` with `press` that also closes it, and one Cancel (a closing button with no
`press`). It is iOS's alert, the web's modal `dialog`, macOS's alert. Open it from an action
with `showModal(id)` (a swipe or menu action asks first), or from a button with
`commandfor=id command="show-modal"`.

```contract
component App
  state items = ["Milk", "Eggs"]
  state aimed = ""
  action ask(item: string)
    aimed = item
    showModal("remove")
  action remove
    items = filter(items, i => i != aimed)
  view
    column padding=16 gap=8
      each i in items key=i
        button press=ask(i) destructive=true testId=`delete-${i}`
          text `Delete ${i}`
      dialog id="remove" role="alertdialog" aria-label=`Delete ${aimed}?`
        text "This can’t be undone."
        button press=remove destructive=true commandfor="remove" command="close" testId="remove-confirm"
          text "Delete"
        button commandfor="remove" command="close" testId="remove-cancel"
          text "Cancel"
```

- Its rows are only text, actions and one Cancel; a form is a sheet (`contract vocab modal`).
  In tests, `clock settle` after it opens, then `tap "remove-confirm"`.

More: docs/recipes/navigation-stack.md

## date: the date, the time and the day
names: day, today, clock, time, Date, exactTime

The date is `time.epochAtZero + now`: the reserved source `exactTime()` answers the launch's
epoch and the zone's offset, and `now` is state a `task` refreshes from `performanceNow()`
(milliseconds since launch) with `every(ms, action)`. A day's key is `formatDate(at,
time.utcOffset, "iso")`, `"2026-03-14"` in the person's own zone. The action that writes an
entry keys it from the instant of the write, not from the ticked `now`, which is display only
and can be a minute behind: just after midnight it is still yesterday.

```contract
shape Time
  epochAtZero: number
  utcOffset: number

component App
  state now = 0
  state days = []
  resource time = exactTime() as shape Time
  derive shown = time.epochAtZero + now
  derive today = formatDate(shown, time.utcOffset, "iso")
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
  action log
    let at = time.epochAtZero + performanceNow()
    now = performanceNow()
    days = concat(days, [formatDate(at, time.utcOffset, "iso")])
  view
    column padding=16 gap=4
      text formatDate(shown, time.utcOffset, "medium") testId="date"
      text formatTime(shown, time.utcOffset, "short") testId="time"
      text today color="-exact-secondary-label" testId="day"
      button press=log testId="log"
        text "Log"
```

- Store `at`, never `performanceNow()` alone: it restarts at 0 every launch. Tick a minute
  for a date, a second or less for a clock. Yesterday is `formatDate(shown - 86400000, …)`;
  keys compare as strings. After a DST change or a new zone, `utcOffset` is answered again
  before the next timer fires.
- Tests pin the clock: `epoch "2026-03-15T06:30:00Z"`, `time-zone "America/Los_Angeles"`,
  then `clock +3600000`; `clock +40000` past midnight, then a `tap`, writes before the tick.

More: docs/recipes/dates-and-days.md, docs/recipes/timers.md

## persist: keep a setting across launches
names: persisted, remember, localStorage, AsyncStorage, UserDefaults

A root `state` that ends in `persist` is read before the first frame and saved after every
change (the page's `localStorage`, `UserDefaults` on Apple): no data module, grant or JSON.

```contract
component App
  state goal = 2000 persist
  state units = "ml" persist
  action setGoal(g: number)
    goal = g
  view
    column padding=16 gap=8
      text `Goal: ${goal} ${units}` testId="goal"
      button press=setGoal(2500) testId="goal-2500"
        text "Set 2,500"
```

- Only a number, string or bool, or an option or list of one, up to 16 KB; records are app
  data, kept by a source (docs/recipes/files-and-grants.md). Only the root's state
  persists, keyed by its name. A test's `reload` relaunches on the same store; a drive
  keeps it across runs with the same `--storage s1`.

More: docs/recipes/persisted-setting.md

## swipe: a list row's swipe actions
names: swipe-actions, swipeActions, swipe-to-delete, swipeleft

The row is a horizontal snap `scroll` naming its content (`swipeContent`) and its trailing
action (`swipeTrailing`, a button's `id`). On iOS that is UIKit's own swipe action (a full
swipe performs the first); on the web the row scrolls to show it.

```contract
component App
  state items = ["Milk", "Eggs"]
  action remove(item: string)
    items = filter(items, i => i != item)
  view
    column
      each i in items key=i
        scroll swipeContent=`row-${i}` swipeTrailing=`delete-${i}` width="100%"
          overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
          row width="100%"
            row id=`row-${i}` width="100%" flex-shrink=0 scroll-snap-align="start" padding="12px 16px"
              text i
            button id=`delete-${i}` destructive=true press=remove(i) aria-label=`Delete ${i}` testId=`delete-${i}`
              flex-shrink=0 scroll-snap-align="start"
              image "symbol:delete"
```

- In a grouped list (`list appearance="auto"`), the scroll is a section's row: leave its
  `width` unsaid and give it `padding-right=0`. To ask first, the action's `press` opens a
  confirmation (`contract vocab confirm`). In tests, `tap "delete-Milk"` presses it.

More: docs/recipes/list-rows.md (search, filter, the long-press menu)

## task: a timer, once or repeating (every, after)
names: every, after, timer, interval, setInterval, setTimeout, debounce

A root `task` has one schedule: `every(ms, action)` repeats, `after(ms, action)` fires once,
`every(frame, action)` ticks once a display frame. `task t mount` runs from launch; `task t
when cond` only while `cond` holds, and `key=x` restarts it when `x` changes. The interval is
a literal number of milliseconds, and the action takes no parameters.

```contract
component App
  state n = 0
  task t mount
    every(1000, tick)
  action tick
    n = n + 1
  view
    text `${n}s` testId="n"
```

- A `derive` that reads `performanceNow()` is not read again as time passes: the tick's action
  writes it to state. In a test `clock +N` fires the timers; never wait in real time.

More: docs/recipes/timers.md

## mutation: a write to the data module, and its reply (send)
names: send, mutate, post, put, patch, request

`mutation name as shape T` is the reply slot, an option (`none` until the reply lands);
`send name = source(args)` in an action asks the data module's source. `pending(name)` is true
while it is in flight; `refreshes res` asks a resource again when the reply lands, `then act`
runs an action (no parameters) after it, `queue` keeps sends in order.

```contract
shape Reply
  ok: bool

component App
  resource items = loadItems() as shape list<string> else empty()
  mutation added as shape Reply refreshes items
  action add
    send added = addItem("Milk")
  view
    button press=add disabled=pending(added) testId="add"
      text `Add (${length(items)})`
```

- One send of a mutation per path (`analyze-send-twice`): a second write needs its own mutation or
  `queue`. Read the reply with `match added { case some(r) => r.ok, case none => false }`.

More: docs/recipes/after-a-write.md

## resource: a read from the data module
names: source, sources, fetch, load, useEffect, useQuery, query

`resource name = source(args) as shape T else empty()` asks the source in `app.ts` and shows its
answer as `name`; the arguments are state, so a change asks again. Until the first answer it is
`else empty()`, the shape's zero (`[]`, `""`, `0`, `false`). `pending(name)` is true while a
request is in flight, `failed(name)` when it had no answer. Only the root declares one.

```contract
component App
  state q = ""
  resource hits = search(q) as shape list<string> else empty()
  action edit(v: string)
    q = v
  view
    column gap=8
      input value=q input=edit aria-label="Search" testId="q"
      text `${length(hits)} found` testId="n"
```

- The source answers exactly the shape (an extra field fails it), and a failed fetch is data, not
  an exception. A child takes what it shows as props; `refresh name` in an action asks again.

More: docs/recipes/fetch-loading-error.md

## match: take an option apart (some, none)
names: case, option, optional, null, undefined, nullable

There is no `null` and no truthiness: a value that may be missing is an `option<T>`, `some(x)` or
`none`, and `match` takes it apart with both arms, each once: as an expression, as a statement
in an action (`match o`, then indented `case some(v)` and `case none`), or as a view node. A
mutation's reply, `first(xs)` and `parseNumber(s)` are options.

```contract
component App
  state picked = none
  derive label = match picked { case some(p) => `Picked ${p}`, case none => "Nothing yet" }
  action pick
    picked = some("tea")
  view
    button press=pick testId="pick"
      text label testId="label"
```

- `when x` needs a bool, never an option: take it apart with `match`, or compare it.

## each: one node per item
names: for, forEach, v-for, list-render, index

`each item in items key=item.id` repeats its nodes once per item. The key, a string, number or bool,
unique and stable, is the row's identity: its state, focus and animation follow the item. A second
name binds the index: `each s, i in items key=s`. An action has no loops: reshape a list with `map`
and `filter` in a `derive`, or in the data source.

```contract
component App
  state items = ["Milk", "Eggs"]
  view
    column gap=4
      each s, i in items key=s
        text `${i + 1}. ${s}` testId=`row-${i}`
```

- A key that is the index (`key=i`) gives a row that moves or is deleted the wrong state: key by an id.

## when: show nodes only under a condition (else, else when)
names: if, else, conditional, v-if, ternary

`when cond` over indented nodes shows them while `cond` is true; `else when cond` and `else` follow
at its indent. The condition is a bool, with no truthiness (`length(xs) > 0`, not `xs`).
A view chooses with `when`, an action with `if`, an expression with `cond ? a : b`.

```contract
component App
  state n = 0
  action more
    n = n + 1
  view
    column gap=8
      when n == 0
        text "Nothing yet"
      else when n < 3
        text `${n} so far`
      else
        text "Plenty"
      button press=more testId="more"
        text "More"
```

- A view never uses `if`, and a component's view root is an element: wrap a `when` in a `column`.

## let: a name inside an action
names: local, const, var, variable

An action reads the state it started with, and its writes commit together, so `doubled = count * 2`
after `count = count + 1` uses the old `count`. `let next = count + 1` binds a local, read after
its declaration, in its block, never reassigned, and not a name already in scope.

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

- A `let` reassigned (`type-let-reassign`) is a `state`, or choose the value where it is bound.

## fn: a function you write
names: function, functions, helper, util

`fn name(a: T, b: U): V = expr` at the top of a file is an effect-free, nonrecursive expression, called
positionally from a view, a `derive` or an action. It reads no state: pass what it needs. A call
of an action gives no value, so a value computed in more than one place is a `fn`.

```contract
fn tax(price: number, pct: number): number = price * pct / 100

component App
  state price = 40
  derive total = price + tax(price, 8)
  view
    text `${total}` testId="total"
```

- No loops: a list is reshaped with `map`, `filter`, `concat` and `slice`.

## shape: a record type, and the type of a data answer
names: record, struct, interface, typedef, model

`shape Item` over indented `field: type` lines declares a record; a type is `number`, `string`, `bool`,
`option<T>`, `list<T>` or another shape. A record is built with every field named, `Item(id="1", ...)`,
and copied with one base and the changes, `Item(item, done=true)`. A resource's `as shape T` and a data
source's answer are this type.

```contract
shape Item
  id: string
  title: string
  done: bool

component App
  state item = Item(id="1", title="Milk", done=false)
  action toggle
    item = Item(item, done=not item.done)
  view
    button press=toggle testId="item"
      text `${item.title}: ${item.done ? "done" : "to buy"}`
```

- No optional fields or methods: a field that may be missing is an `option<T>`.

## props: a component's inputs, and its children
names: prop, children, slot, inject, provide, component

A child `component` declares `props` as indented `name: type` lines (a type `action` takes a
handler) and is used with named arguments, `Tag(label="Tea", pick=choose("Tea"))`. The root is
the file's first component. A `slot` in a child and `children` in its view place the nodes the
caller indents under the use. Props are read-only.

```contract
component App
  state chosen = ""
  action choose(t: string)
    chosen = t
  view
    column gap=8
      Tag(label="Tea", pick=choose("Tea"))
      text chosen testId="chosen"

component Tag
  props
    label: string
    pick: action
  view
    button press=pick testId=`tag-${label}`
      text label
```

- Resources, mutations and tasks are the root's: a child takes their results as props.

## derive: a value computed from state
names: computed, memo, useMemo, selector, getter

`derive name = expr` is recomputed when what it reads changes. It is read-only, never assigned in an
action, and not state: whatever can be calculated from state is a derive, not a second `state`
kept in step. Cycles are refused.

```contract
component App
  state nums = [1, 5, 9]
  state min = 4
  derive big = filter(nums, n => n >= min)
  view
    text `${length(big)} of ${length(nums)}` testId="n"
```

- A derive is no timer: one that reads `performanceNow()` shows the time of its last recompute.

## action: what a press, an input or a key does
names: handler, onClick, onPress, onChange, setState, reducer

`action name(params)` over indented statements: `x = expr` on its own state, `let`, `send`, `refresh`,
`if`/`else`, `match`, a host command, or a call of another action. No loops, awaits or returns; the
writes commit together. An event passes the arguments the handler captured, then its payload:
`press=add(1)`, `input=edit` (the new text), `change=toggle(i.id)` (the new bool).

```contract
component App
  state n = 0
  action add(by: number)
    n = n + by
  view
    row gap=8
      button press=add(1) testId="inc"
        text "+1"
      button press=add(-1) testId="dec"
        text "-1"
      text `${n}` testId="n"
```

- An action reads the state it started with: bind a value with `let` to read it after writing it.

## stepper: a count with plus and minus (a number field)
names: UIStepper, NSStepper, quantity, spin-button, increment, counter

`input type="number"` with `min`, `max` and `step` is a numeric field with the platform's stepping control
(UIStepper on iOS, NSStepper on the Mac, the browser's spin buttons on the web); the field stays
editable; `appearance="textfield"` drops the control. A count 1-99:

```contract
component App
  state qty = "1"
  action edit(v: string)
    qty = v
  view
    input type="number" min=1 max=99 value=qty input=edit aria-label="Quantity" testId="qty"
```

- The value is text, as any field's: a step is an `input` and a `change` with the new text, which a
  derive parses with `parseNumber` (docs/recipes/number-field.md). `step` defaults to 1.
