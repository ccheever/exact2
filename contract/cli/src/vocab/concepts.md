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
  does, with nothing to write: the first route's rows push and draw the `forward-chevron`. A
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
(`contract vocab confirm`). A `dialog` you lay out yourself opens on the web and macOS but
not on iOS, and `role="dialog"` on a `column` gets no Escape, focus or sheet anywhere.

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
names: day, today, clock, time, Date, exactTime, every

The date is `time.epochAtZero + now`: the reserved source `exactTime()` answers the launch's
epoch and the zone's offset, and `now` is state a `task` refreshes from `performanceNow()`
(milliseconds since launch) with `every(ms, action)`. A day's key is `formatDate(at,
time.utcOffset, "iso")`, `"2026-03-14"` in the person's own zone.

```contract
shape Time
  epochAtZero: number
  utcOffset: number

component App
  state now = 0
  resource time = exactTime() as shape Time
  derive at = time.epochAtZero + now
  derive today = formatDate(at, time.utcOffset, "iso")
  task tick mount
    every(60000, step)
  action step
    now = performanceNow()
  view
    column padding=16 gap=4
      text formatDate(at, time.utcOffset, "medium") testId="date"
      text formatTime(at, time.utcOffset, "short") testId="time"
      text today color="-exact-secondary-label" testId="day"
```

- Store `at`, never `performanceNow()` alone: it restarts at 0 every launch. Tick a minute
  for a date, a second or less for a clock. Yesterday is `formatDate(at - 86400000, …)`;
  keys compare as strings.
- Tests pin the clock: `epoch "2026-03-15T06:30:00Z"`, `time-zone "America/Los_Angeles"`,
  then `clock +3600000`.

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
