<!--
`contract vocab <name>` answers each concept below with its whole idiom (LLP 1116.004 D5.1):
what a builder looked up a recipe or the grammar for, answered in one lookup. A section is
`## <name>: <what it does>`, then `names: …`, the other names it answers to, then the answer:
a sentence or two, the minimal complete code, and the recipe for depth. A name that is also a
tag, attribute or function prints its entry first, then the concept. Every `contract` block
compiles (`contract/cli/tests/it/vocab.rs`).
-->

## scroll: scrolling to the top, or to an item
names: scrollTo, scrollToTop, scroll-to-top, scrollToIndex, scrollIntoView, scrollTop, scrollToEnd, scrollToBottom, scroll-to-bottom

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
        for r in rows key=r
          text r id=`row-${r}` padding=16
```

- Every scroller above the element moves, so a horizontal strip inside a sheet or a form
  scrolls the form too. `block="nearest"` leaves a scroller that already shows the element
  where it is, and `inline` (`"start"`, `"center"`, `"end"`, default `"nearest"`) places it
  across a horizontal strip: ``scrollIntoView(`icon-${i}`, block="nearest", inline="start")``.
- A virtualized `list` (rows not built yet) scrolls to a row by its key:
  `scrollIntoView("list-id", key, block="center")`. A chat opens at its newest row with
  `scroll-start="end"` on a `list virtualized=true`, and `scrollFollowEnd=true` keeps it
  there as messages arrive, while the reader is at the end (`chat-thread`; a grouped list
  is never virtualized).
  To restore after remount, save `ScrollEvent.firstVisibleKey` and
  `firstVisibleOffset` in the app, then call `scrollIntoView("list-id", key, offset=within)`.
  The optional key is text; the offset is signed CSS pixels into the row on the list's axis.
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
  titles: string[]

component App
  resource feed = loadFeed() as shape Feed else empty()
  action reload
    refresh feed
  view
    column padding=16 gap=8
      if failed(feed) or feed.error != ""
        text "Couldn’t load the feed." testId="feed-error"
        button press=reload testId="retry"
          text "Try Again"
      else if not feed.loaded
        progress aria-label="Loading" testId="feed-loading"
      else if feed.titles.length == 0
        text "Nothing here yet." color="-exact-secondary-label" testId="feed-empty"
      else
        for t in feed.titles key=t
          text t
```

- The build asks each source once and bakes its answer into the first frame. Storage,
  the network and the native module refuse it there with `code: 'bake'`: let that throw (`catch (e) { if (e.code
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
names: segment, segments, radiogroup, radio, segmented, segmented-control, segmentedcontrol, UISegmentedControl

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
        for p in [15, 18, 20, 25] key=p
          button role="radio" aria-checked=(tip == p) press=pick(p) testId=`tip-${p}` flex=1
            text `${p}%`
      text `Tip: ${tip}%` testId="tip"
```

- Each option is one short `text` (or a symbol `image` with an `aria-label`); leave its
  look unsaid. A radio that paints itself (its own `background-color`, `border` or
  `border-radius`, LLP 1104 D2) makes the group your drawn control on every host, keys
  and all; `appearance="none"` on the `radiogroup` says so whatever the radios write. Six or more options, or long labels: a `select`. On or off: `input
  type="checkbox" switch`.
- `state tip = 18 persist` keeps the choice across launches.

More: docs/recipes/persisted-setting.md

## screen: a second screen, a list and its detail, and Back
names: navigate, navigation, navigationBack, back, detail, route, routes, router, list-detail, master-detail, deep-link, deep link, protocol_handlers, openURL, url-scheme, react-router, back button, back-button, back title, backButtonDisplayMode, headerBackTitle, backButtonTitle, backBarButtonItem, navigationBackTitle, large title, large-title, largeTitle, prefersLargeTitles, navigation bar, navigation-bar, navbar, UINavigationBar, phase, stage

A second screen is a route. `routes` gives each screen a path, the root draws `stack(nav)`, a
row `push`es its detail and `back` returns. That gives the web its Back (the browser's Back
pops the stack; Forward comes through `navigate`), a reload that keeps the place and a link to
every screen (`/recipe/Ramen` opens on that recipe, the list under it), and iOS its navigation
bar, Back button and edge swipe. Screens swapped by `if showingDetail` get none of these
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
      for e in stack(nav) key=e.id
        if e.name == "recipes"
          column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0 display="flex" flex-direction="column"
            header padding="8px 16px"
              text "Recipes" role="heading" aria-level=1
            list id="list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
              section
                for n in ["Pancakes", "Ramen", "Tacos"] key=n
                  button press=open(n) testId=`recipe-${n}`
                    text n
                    image "symbol:forward-chevron"
        if e.name == "recipe"
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
- The chevron is the row's last child with no colour or size of yours: UIKit's grey
  disclosure. A row that wraps (`flex-wrap`) at large text sizes wraps its parts in a `row
  flex=1 min-width=0 flex-wrap="wrap"` and leaves the chevron after it, at the trailing edge.
- Each route keeps its own scroll: a pushed screen opens at its top, and Back returns to the
  list where it was left. What an `if` swaps inside one route's scroll keeps that scroll's
  place, as a web page's does, so the stages a person moves through (a quiz's start, its
  questions, its results) are routes too; swapped by a state, the next stage opens where the
  last was scrolled to, part way down and under the bar. Build locations with
  `path("recipe", name)`; a route's parameters are strings (`e.params.name`). In tests, `clock settle` after each push and back. A form
  over the screen is a sheet (`contract vocab modal`).
- A welcome shown at first launch only is not a second route: the main screen is the root
  route and shows the welcome while a `persist` flag is unset (docs/recipes/first-launch.md).
  A stack's first visit keeps its root route: `replace` to another route is refused there.
- A route's first child, a `header` holding one heading, is iOS's navigation bar: an
  `aria-level=1` heading is its large title, a lower level the inline title. A pushed route
  has the platform's Back button and edge swipe with nothing written; a control whose `id`
  is the root's `navigationBack` runs your action and titles the button. Draw neither
  (docs/contract-for-agents.md, the table of what each host draws).
- The Back button's title: exact2 leaves UIKit's default back button, which on iOS 26 and
  later is a bare chevron. For "‹ Settings", put a
  `button id="back" press=back` with `text "Settings"` (no `‹`: iOS draws it) in the pushed
  route's `header`, with `navigationBack="back"` on the root.
- A deep link (`protocol_handlers` in app.json, docs/recipes/app-json.md) arrives at `navigate`.
  A drive delivers one as the system would, without iOS's "Open in …?" alert: `bun exact.mjs
  agent ios "type <root> euxy://p/abc"`, the root being the node with `navigate=` (macOS and
  iOS; a path such as `/recipe/Ramen` works on every host).

More: docs/recipes/navigation-stack.md (a list, its detail, a create sheet, a delete alert)

## notfound: a location no route matches (404, an unknown page)
names: not-found, 404, unknown-route, unknown-url, missing-page

A location no route matches (a mistyped link, a stale bookmark) keeps its URL and shows "Page not
found" with a Go Home link: with no `notfound` line in `routes`, the compiler writes it and that
screen into your `for e in stack(nav)`. Write `notfound` (last, no path) to draw your own, a
route like any other. A detail whose id names nothing (`/recipe/999`) matches its route: say "No
such recipe" there, with a way back (`pattern list-detail`).

```contract
routes nav
  recipes "/"
  notfound

component App
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0
          if e.name == "notfound"
            text `Nothing at ${e.url}` testId="missing"
```

## modal: a form over the screen, with Save and Cancel
names: dialog, sheet, closedby, popup, grabber, sheet-grabber, prefersGrabberVisible, sheetCornerRadius, preferredCornerRadius

A form you lay out (fields, Save, Cancel) is a route presented as a sheet:
`navigationPresentation="modal"` on its root, its Cancel the control whose `id` the root's
`navigationBack` names, so the iOS swipe down and the web's Escape press it, and the screen
under it is inert. A question with buttons only is a `dialog role="alertdialog"`
(`contract vocab confirm`). A `dialog` you lay out yourself is a smaller thing: the web's and
macOS's modal `dialog`, and on iOS a sheet as tall as its content, with no bar.
`role="dialog"` on a `column` gets no Escape, focus or sheet anywhere. A `dialog` opens with
`showModal(id)` or `commandfor=id command="show-modal"` and takes no `open`: in HTML `open` alone
shows a dialog non-modally, and a dialog that state holds open is not available yet.

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
    name = draft.trim()
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        if e.name == "home"
          column navigationKey=`${e.id}` position="absolute" inset=0 padding=16 gap=8
            text name testId="name"
            button press=open testId="rename"
              text "Rename"
        if e.name == "rename"
          column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
            header display="flex" align-items="center" justify-content="space-between" padding="8px 16px"
              button id="back" press=back aria-label="Cancel" testId="cancel"
                image "symbol:close"
              text "Rename" role="heading"
              button press=save buttonStyle="bordered-prominent" aria-label="Save"
                disabled=(draft.trim() == "") testId="save"
                image "symbol:checkmark"
            input value=draft input=edit aria-label="Name" autofocus=true margin=16 testId="field"
```

- In tests, `clock settle` after the push and after Save. A sheet's height:
  `navigationDetent="medium"` (or `"fit-content"`, or `"45%"` of the sheet's maximum).
- A sheet's grabber is the platform's: iOS shows one on a sheet with several detents.
  `navigationGrabber="visible"` or `"hidden"` on the route says otherwise; draw none.
- A sheet's corners are the platform's: on iOS 26 and later a large sheet is rounded like
  the display and floats inset from the bottom edge, as UIKit's own does. A `border-radius`
  on the route is its corner radius (UIKit's `preferredCornerRadius`, React Native's
  `sheetCornerRadius: 40` is `border-radius=40`), which also runs a large sheet to the
  bottom edge.

More: docs/recipes/navigation-stack.md (a list, its detail, a create sheet, a delete alert)

## confirm: ask before what can't be undone (delete, close, cancel)
names: confirmation, alert, alertdialog, delete-confirmation, Alert, showModal, prompt, rename, action sheet, action-sheet, actionSheet, ActionSheetIOS, showActionSheetWithOptions, UIAlertController

A `dialog role="alertdialog"`: its `aria-label` the question, a `text` the message, each action
a `button` with `press` that also closes it, and one Cancel (a closing button with no
`press`). It is iOS's alert, the web's modal `dialog`, macOS's alert. Open it from an action
with `showModal(id)` (an action that asks first), or from a button with
`commandfor=id command="show-modal"`. It takes no `open`: in HTML `open` alone shows a dialog
non-modally, and a dialog that state holds open is not available yet.

```contract
component App
  state items = ["Milk", "Eggs"]
  state aimed = ""
  action ask(item: string)
    aimed = item
    showModal("remove")
  action remove
    items = items.filter(i => i != aimed)
  view
    column padding=16 gap=8
      for i in items key=i
        button press=ask(i) destructive=true testId=`delete-${i}`
          text `Delete ${i}`
      dialog id="remove" role="alertdialog" aria-label=`Delete ${aimed}?`
        text "This can’t be undone."
        button press=remove destructive=true commandfor="remove" command="close" testId="remove-confirm"
          text "Delete"
        button commandfor="remove" command="close" testId="remove-cancel"
          text "Cancel"
```

- Ask first for what can't be undone: a detail screen's Delete, Close Poll, Cancel Event, a bulk
  clear. A routine or recoverable step asks nothing: a row's swipe (`contract vocab swipe`), a
  row removed from a form being edited.
- Its rows are only text, actions and one Cancel; a form is a sheet (`contract vocab modal`).
  In tests, `clock settle` after it opens, then `tap "remove-confirm"`.
- A prompt (rename, a name for a new list) is the same alert with an `input` row:
  `input value=draft input=editDraft placeholder="Name"` among its rows. iOS's alert shows it
  as its own text field, macOS's `NSAlert` beneath its message; each keystroke is the input's
  `input`, so the action's `press` reads `draft`. Return presses the first action that is not
  destructive (nothing while it is disabled), unless the field has a `submit`, which then has
  Return alone. `disabled=(draft == "")` on Save dims it while the field is empty.
- An alert that only tells ("Couldn't save.") is a `text` and one closing button, no action:
  the platform's one-button alert. Set the message in state, then `showModal(id)`
  (`contract vocab error-alert`). Leave `closedby` unwritten: iOS has no dismissible form of it.
- An alert has no outside: Escape and Back cancel it (the Mac's one-button alert takes Return,
  not Escape, as AppKit's does), a tap beside it does nothing. Leave
  `closedby` unwritten; `closedby="any"` asks for the platform's dismissible form in its place
  (an action sheet on an iPhone, a dialog a click outside closes on the web).
- Several actions and no `text` make an action sheet (an "Open in…" chooser): from the
  bottom of an iPhone, its Cancel drawn, when it has a Cancel; a popover at its invoker
  when it has none.

More: docs/recipes/navigation-stack.md, docs/recipes/error-alert.md

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
    days = days.concat([formatDate(at, time.utcOffset, "iso")])
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
  keeps it across runs with the same `--storage s1` (on the web, with `--keep-storage` too).

More: docs/recipes/persisted-setting.md

## swipe: a list row's swipe actions
names: delete, swipe-actions, swipeActions, swipe-to-delete, swipeleft, swipeContent, swipeLeading, swipeTrailing, swipe actions

`delete=action` on a list row is its swipe to delete: a red Delete beside the row, which a
full swipe performs. On iOS it is UIKit's own swipe action; on the web and the Mac the row
scrolls to show it. `disabled` beside it keeps that row from deleting, and the row still
opens. The swipe deletes without asking, as iOS does where a deletion is routine or
recoverable (Mail, Notes, Reminders); an irreversible loss confirms, as a detail screen's
Delete or a bulk clear does, its action opening an alert (`contract vocab confirm`). In tests, `tap "item-Milk-delete"` presses it (the row's `testId` and
`-delete`).

```contract
component App
  state items = ["Milk", "Eggs"]
  action remove(item: string)
    items = items.filter(i => i != item)
  view
    list appearance="auto" flex=1
      section
        for i in items key=i
          row delete=remove(i) testId=`item-${i}`
            text i
```

- On a row of a grouped list's `section`, or of a `list` whose rows go down the page;
  anywhere else it does nothing (warning `lower-delete-row`). A `list` with sections is a
  grouped list only with `appearance="auto"` (warning `lower-list-section`).
- Other actions than Delete (Pin, Flag, a leading action, several) are the long form: a
  horizontal snap `scroll` laid out as a row (`display="flex"`), naming its content
  (`swipeContent`) and its actions (`swipeTrailing`, `swipeLeading`: buttons' `id`s, inside
  the scroll after the content), each button with a `press` and a name. Or put them in the
  long-press menu (`contract vocab menu`). A Delete that the platform draws is `delete=`,
  not the long form; the long form is for a Delete you paint (below) or word otherwise, a
  handler that reads the press's event (`delete=` passes none), or a scroll you drive.

```contract
component App
  state items = ["Milk", "Eggs"]
  state pinned = ""
  action pin(item: string)
    pinned = item
  view
    list appearance="auto" flex=1
      section
        for i in items key=i
          scroll swipeContent="row" swipeTrailing="pin" display="flex" padding-right=0
            overflow-x="scroll" overflow-y="hidden" scrollbar-width="none" scroll-snap-type="x mandatory"
            row id="row" width="100%" flex-shrink=0 scroll-snap-align="start" padding-right=16
              box-sizing="border-box"
              text i
            button id="pin" press=pin(i) aria-label="Pin" flex-shrink=0
              scroll-snap-align="start"
              image "symbol:pin"
```

- An action is UIKit's grey unless you colour it: `accent-color` on its button (Flag's
  `-exact-system-orange`, Archive's `-exact-system-indigo`; `destructive=true` is red). Its
  visible text is the label UIKit draws under the glyph: `image` then `text "Flag"` shows
  both, as Mail does; an image with only `aria-label` shows no label.
- UIKit draws its swipe actions itself (rounded on iOS 26 and later), whatever you paint.
  To keep painted actions (flush 76-point rectangles of your own colours), write
  `appearance="none"` on every control the row names: on iOS the row then swipes by its own
  scroll, as on the web, and keeps the swipe's behaviours: one row open at a time, a full
  swipe (past half the row) presses the outermost action, and VoiceOver reads each action as
  a custom action of the row's content.

More: docs/recipes/list-rows.md (search, filter, the long-press menu)

## task: a timer, once or repeating (every, after)
names: every, after, timer, interval, setInterval, setTimeout, debounce

A root `task` has one schedule: `every(ms, action)` repeats, `after(ms, action)` fires once,
`every(frame, action)` ticks once a display frame. `task t mount` runs from launch; `task t
when cond` only while `cond` holds, and `key=x` restarts it when `x` changes. The interval is
a literal number of milliseconds, and the action takes no parameters.

```contract
component App
  state seconds = 0
  task t mount
    every(1000, tick)
  action tick
    seconds = floor(performanceNow() / 1000)
  view
    text `${seconds}s` testId="n"
```

- A `derive` that reads `performanceNow()` is not read again as time passes: the tick's action
  writes it to state. In a test `clock +N` fires the timers; never wait in real time.
- Measure time, never count ticks: on a device, ticks missed while the app was suspended fire
  once, not once each.

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
  resource items = loadItems() as shape string[] else empty()
  mutation added as shape Reply refreshes items
  action add
    send added = addItem("Milk")
  view
    button press=add disabled=pending(added) testId="add"
      text `Add (${items.length})`
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
  resource hits = search(q) as shape string[] else empty()
  action edit(v: string)
    q = v
  view
    column gap=8
      input value=q input=edit aria-label="Search" testId="q"
      text `${hits.length} found` testId="n"
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

- `if x` needs a bool, never an option: take it apart with `match`, or compare it.

## for: one node per item
names: each, forEach, v-for, list-render, index

`for item in items key=item.id` repeats its nodes once per item. The key, a string, number or bool,
unique and stable, is the row's identity: its state, focus and animation follow the item. A second
name binds the index: `for s, i in items key=s`. An action has no loops: reshape a list with `map`
and `filter` in a `derive`, or in the data source.

```contract
component App
  state items = ["Milk", "Eggs"]
  view
    column gap=4
      for s, i in items key=s
        text `${i + 1}. ${s}` testId=`row-${i}`
```

- A key that is the index (`key=i`) gives a row that moves or is deleted the wrong state: key by an id.

## if: show nodes only under a condition (else, else if)
names: when, else, conditional, v-if, ternary

`if cond` over indented nodes shows them while `cond` is true; `else if cond` and `else` follow
at its indent. The condition is a bool, with no truthiness (`xs.length > 0`, not `xs`).
A view and an action choose with `if`, an expression with `cond ? a : b`; `when` is a task's gate.

```contract
component App
  state n = 0
  action more
    n = n + 1
  view
    column gap=8
      if n == 0
        text "Nothing yet"
      else if n < 3
        text `${n} so far`
      else
        text "Plenty"
      button press=more testId="more"
        text "More"
```

- A component's view root is an element: wrap an `if` in a `column`.

## let: a name inside an action
names: local, const, variable

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
`option<T>`, `T[]` or another shape. A record is built with every field named, `Item(id="1", ...)`,
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

- A shape has no optional fields and declares no methods: a field that may be missing is an `option<T>`.

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
  derive big = nums.filter(n => n >= min)
  view
    text `${big.length} of ${nums.length}` testId="n"
```

- A derive is no timer: one that reads `performanceNow()` shows the time of its last recompute.

## action: what a press, an input or a key does
names: state, useState, handler, onClick, onPress, onChange, setState, reducer

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

- `state name = value` is what actions write; `persist` keeps it across launches.
- An action reads the state it started with: bind a value with `let` to read it after writing it.

## stepper: a count with plus and minus (a number field)
names: UIStepper, NSStepper, quantity, spin-button, increment, counter

`input type="number"` with `min`, `max` and `step` is a field with the platform's stepper
(UIStepper, NSStepper, the browser's spin buttons); the field stays editable;
`appearance="textfield"` drops the stepper. A count 1-99:

```contract
component App
  state qty = "1"
  action edit(v: string)
    qty = v
  view
    input type="number" min=1 max=99 value=qty input=edit aria-label="Quantity" testId="qty"
```

- The value is text, as any field's: a step is an `input` and a `change` with the new text, which a
  derive parses with `parseNumber` (`number-field`).
- A press adds `step` (1 unsaid), the value's unit: `step=50` for millilitres. A freely typed
  amount has no stepper: a plain `input` with `inputmode`.
- On iOS the stepper needs the field about 130 pt wide (`flex=1` in a row), else the field shows alone.
- A test types the value (`type "qty" "5"`); `agent … layout qty` gives `native.stepper`'s
  `increment` point, and `tap qty at <x> <y>` presses it.


## gradients: CSS backgrounds and masks
names: background-image, mask-image, repeating-linear-gradient

`background-image` takes `none` or up to four layers of `linear-gradient()`,
`radial-gradient()`, `conic-gradient()` and `repeating-linear-gradient()`.
`mask-image` takes one gradient. Each takes at most 64 colour stops.
Linear/radial stops take percentages, px lengths, or unitless zero, with two
positions allowed per colour. Omitted and mixed-unit stops are fixed up as
CSS specifies once the box size is known. Conic stops take percentages or angles.
Percentages outside 0–100%, negative radial lengths, other length units,
`calc()`, colour hints, repeating radial/conic gradients, `url()`, `image-set()`
and `cross-fade()` remain refused.

```contract
component Hatch
  view
    view width=320 height=200 background-color="#fcfcfc" background-image="repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)"
```

The period is the distance between the first and last fixed-up stops, measured
along the gradient line. `background-attachment` takes `scroll` or `fixed`;
`local` remains deferred. Each row paints its own background, so a repeated
hatch on separate rows does not establish one pattern across the list.

## menu: a pull-down menu of actions, and a long-press one
names: menus, popover, dropdown, pulldown, pull-down, popupmenu, menu at the click, popup at point, contextPopover, UIMenu, context menu, context-menu, contextMenu, UIContextMenuInteraction, menuitemradio, menuitemcheckbox, checkmark

A menu is HTML's: a `column popover="auto" role="menu"` of `button`s, opened by a button's
`popovertarget` (on iOS a `UIMenu`; in a `header`, the bar item's pull-down). A long-press
menu is the same column, named by `contextPopover` on what is pressed, with
`contextmenu=` an action that notes which item it was. Each button closes it with
`popovertarget="<menu id>" popovertargetaction="hide"`.

```contract
component App
  state picked = ""
  state aimed = ""
  action sort(by: string)
    picked = by
  action aim(id: string)
    aimed = id
  action remove
    picked = `removed ${aimed}`
  view
    column
      button popovertarget="sort-menu" aria-label="Sort" testId="sort"
        image "symbol:more"
      column id="sort-menu" popover="auto" role="menu"
        button press=sort("name") popovertarget="sort-menu" popovertargetaction="hide" testId="by-name"
          text "Name"
        button press=sort("date") popovertarget="sort-menu" popovertargetaction="hide"
          text "Date"
      text "Row 1" contextmenu=aim("1") contextPopover="row-menu" testId="row-1"
      column id="row-menu" popover="auto" role="menu"
        button press=remove destructive=true popovertarget="row-menu" popovertargetaction="hide"
          text "Remove"
      text picked testId="picked"
```

- The menu's title is the popover column's `aria-label` (iOS: `UIMenu(title:)`).
- A checkmark: `role="menuitemradio" aria-checked=(sortKey == "name")` on the row (a toggle:
  `role="menuitemcheckbox"`).
- A row's second `text`, under the first (a block `button`, or `flex-direction="column"`), is
  its grey subtitle: `text "Name"` then `text "A to Z"`. Side by side they stay one title.
- An `hr` between rows starts a section; `destructive=true` paints a row red.
- At the click, as Electron's `Menu.popup({x, y})`: the button's `press` action takes
  `e: MouseEvent`, keeps `e.clientX`, `e.clientY` (both 0 from a key; then leave the insets
  `auto`), and calls `showPopover("menu", source="button")`; the popover writes them as `left`
  and `top` with `margin=0` and no `position-area`. iOS keeps the menu on its invoker (LLP 1021 D2).

A test opens one with `tap "sort"`, a long-press one with `tap "row-1" contextmenu`, then
taps the item. A row whose `popovertarget` names another menu is its submenu.

## search: the bar's search field, filtering a list or querying a server
names: searchable, searchbar, search-bar, UISearchBar, UISearchController, search bar, search field, hidesSearchBarWhenScrolling, hideWhenScrolling, sticky search

An `input type="search"` in a route's `header` is the bar's search field: on iOS UIKit's
search controller (a magnifier, a clear button, Cancel), on the web a field under the title.
Each keystroke is its `input`, and the list filters on the query, so there is no Search
button. No match is an `EmptyState` in the list's place (`pattern list-detail`).

```contract
routes nav
  home "/"

component App
  state query = ""
  state notes = ["Groceries", "Ideas", "Recipes"]
  derive shown = notes.filter(n => n.toLowerCase().includes(query.trim().toLowerCase()))
  action search(v: string)
    query = v
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="notes" position="absolute" inset=0
          header display="flex" flex-wrap="wrap" align-items="center" gap=8 padding="8px 16px"
            text "Notes" role="heading" aria-level=1 flex=1
            input type="search" value=query input=search placeholder="Search" aria-label="Search Notes"
              testId="search" width="100%"
          list id="notes" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              for n in shown key=n
                row
                  text n
```

- On iOS the field scrolls away with the content while it is empty, as UIKit's does;
  `position="sticky" top=0` on the `input` keeps it shown (`hidesSearchBarWhenScrolling =
  false`, React Navigation's `hideWhenScrolling={false}`).
- `flex-wrap` and `width="100%"` give the field its own line on the web; iOS places it
  itself. A field in the content with a Search button beside it is a web form's, a tell.
- A test types the query: `type "search" "gro"`, and `type "search" ""` to clear it.

The same field queries a server when a `resource` takes the query as its argument: each
change asks again (`contract vocab resource`), and the list shows the answer. To ask only on
the keyboard's Search key (Return on the web and the Mac), as for a slow or costly server,
the field's `submit` copies the query into the state the resource reads:

```contract
routes nav
  home "/"

shape Book
  id: string
  title: string

component App
  state query = ""
  state asked = ""
  resource found = searchBooks(asked) as shape Book[] else empty()
  action search(v: string)
    query = v
  action ask
    asked = query.trim()
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="books" position="absolute" inset=0
          header display="flex" flex-wrap="wrap" align-items="center" gap=8 padding="8px 16px"
            text "Books" role="heading" aria-level=1 flex=1
            input type="search" value=query input=search submit=ask placeholder="Search"
              aria-label="Search Books" testId="search" width="100%"
          list id="books" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              for b in found key=b.id
                row
                  text b.title
```

- Live, as you type: `resource found = searchBooks(query)`, with no `submit`. A test types
  the query, then `clock data` before it reads the answer; on the Search key, `type "search"
  key "Enter"`.

More: docs/recipes/list-rows.md

## menubar: adding commands to the Mac menu bar
names: -exact-menu, -exact-menu-remove, -exact-menu-keys

On a button, `-exact-menu` is a literal `about` (directly under About), `app`,
`file`, `edit`, `view`, `go`, `window`, `help` or `none` (no item; the chord
stays a shortcut). `dock` beside one of them, or alone, also files the button
in the app's Dock menu (`-exact-menu="file dock"`). `status` alone presents the
button as the app's status item: a menu bar item on macOS, a notification-area
icon on Windows; it takes the button's place, its `popovertarget` menu is the
item's menu, and with another id it fails `lower-menu-status` (LLP 1133.025).
It adds an item even without a chord and overrides
`aria-keyshortcuts` chord filing. The item's label is the button's accessible
label or text; `disabled` disables it. An item in the app menu or Help is the
app's: enabled with no key window, under a sheet and under a Contract modal dialog.
Hidden/removed buttons leave the menu. Other hosts keep the ordinary button
and ignore the hint. The agent taps the button. A root
`-exact-menu-remove="show-tab-bar autofill close-all"` removes selected
standard items and their menu equivalents. The fixed ids are `close-all`, `autofill`, `show-tab-bar`, `show-all-tabs`, `enter-full-screen`,
`minimize`, `zoom`, `bring-all-to-front`, `show-previous-tab`, `show-next-tab`,
`move-tab-to-new-window`, `merge-all-windows`, `back` (Go ▸ Back), `develop` (the
Develop menu; its chords stay), `paste-and-match-style`, `find`,
`spelling-and-grammar`, `substitutions`, `transformations`, `app-help` (Help ▸
*App* Help), `actual-size`, `zoom-in`, `zoom-out`, and `status-quit` (the
host's Quit ending a status item's menu).
The whitespace-separated list must be literal; unknown ids and nested use fail
with `lower-menu-remove`, naming the set. Empty removes nothing; duplicates
are harmless. The view’s root is a node, not a conditional. Root replacement and
window switching restore the appropriate menu. App buttons and other standard
items remain. Other hosts ignore removal (LLP 1021 §5.3).
A root `-exact-menu-keys="close-window=none"` keeps File ▸ Close Window with no
chord, so ⌘W reaches the page; declare `-exact-menu-remove="close-all"` with it,
or AppKit's Close All ⌥⌘W shows as a row of its own. `close-window=Meta+Shift+W`
gives it ⇧⌘W. `close-window` is the one id; the chord is one `aria-keyshortcuts`
chord with `Meta` that is not the host's or the system's own (⌘Q, ⌘W, ⌘H, ⌘M,
⌘-comma and the like). Another id, an id named twice, a chord without `Meta`, a
standard chord and nested use fail with `lower-menu-keys`. An app command with
the chord still takes it; without the declaration Close Window has ⌘W (LLP 1021
§5.3.3).

```contract
component App
  state message = ""
  action checkUpdates
    message = "Checking…"
  view
    column -exact-menu-remove="show-tab-bar autofill close-all"
      button "Check for Updates…" -exact-menu="app" press=checkUpdates testId="updates"
      text message
```
## ::placeholder: a text field's placeholder colour
names: placeholder, placeholder-color, -exact-placeholder-color, placeholderTextColor, ::before, ::after

CSS's `::placeholder { color }` is a block: `::placeholder` on its own line under an `input` or a
`textarea` (before its children; there a value may compute) or in a `style`, then `color=…`
indented beneath it. It admits `color` alone, with its own alpha; the field's own block wins over its
class's; without one each platform keeps its own placeholder colour (UIKit's, AppKit's, the
browser's).

```contract
style Field
  ::placeholder
    color="#6b7280"

component App
  state dark = false
  view
    column
      input placeholder="Search" class=Field
      textarea placeholder="Notes"
        ::placeholder
          color=(dark ? "#9ca3af" : "#6b7280")
```

- Another property in the block (`font-style`, `opacity`) is refused; so is the block on any other
  element, which has no placeholder.
- `::before` and `::after` are refused: Contract has no generated box. Write what they would show as
  a child (a `text`, an `image`).

## glass: Liquid Glass, a material or a blur behind a box
names: Liquid Glass, liquid-glass, liquidGlass, material, materials, vibrancy, blur, frosted, UIVisualEffectView, UIGlassEffect, BlurView, GlassView, expo-blur, expo-glass-effect

Bars, tab bars, sheets and menus are the platform's, glass included: leave them unsaid. A
surface of your own takes `backgroundMaterial`: `"glass"` or `"glass-clear"` is
`UIGlassEffect` on iOS 26 and `NSGlassEffectView` on macOS 26; `"ultra-thin"`, `"thin"`,
`"material"`, `"thick"` and `"chrome"` (each with `-light` and `-dark`) are UIKit's blurs,
and `"sidebar"`, `"menu"`, `"popover"`, `"hud-window"` the Mac's. The web and Linux draw a
blur that approximates each. A glass button is `buttonStyle="glass"` or
`"prominent-glass"`, and `glassGroup=<points>` on a box merges the glass inside it as
SwiftUI's `GlassEffectContainer` does.

```contract
component App
  view
    column height="100%" background-color="-exact-system-teal"
      row backgroundMaterial="glass" border-radius=24 margin=16 padding=12 gap=8
        button buttonStyle="glass" aria-label="Play" testId="play"
          image "symbol:play"
        text "Now playing"
```

- CSS's `backdrop-filter="blur(12px) saturate(1.8)"` blurs what is behind on the web, macOS
  and Linux; iOS draws a fixed light material for it, so a surface meant for iPhone says
  `backgroundMaterial`, which wins when both are written.
- A navigation bar's buttons share one glass capsule on iOS 26; `buttonStyle="glass"` on
  each gives it a capsule of its own (`contract vocab bar-buttons`).

## bar-buttons: the navigation bar's buttons, grouped or each in its own capsule
names: bar button, bar-button, barbuttons, UIBarButtonItem, sharesBackground, headerRight, headerLeft, toolbarItem, ToolbarItem, ToolbarItemGroup

The buttons of a route's `header` are its bar's items: those before the heading lead, those
after it trail. iOS 26 puts adjacent ungrouped buttons on one shared glass capsule.
`buttonStyle="glass"` on a bar button gives it a capsule of its own (`sharesBackground =
false`), and a `row role="group"` around some keeps those on one capsule, set apart from the
next group. Leave their size, colour and padding unsaid.

```contract
routes nav
  home "/"

component App
  state n = 0
  action add
    n = n + 1
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0
          header
            text "Patterns" role="heading" aria-level=1
            button popovertarget="sort-menu" buttonStyle="glass" aria-label="Sort" testId="sort"
              image "symbol:sf/arrow.up.arrow.down"
            button press=add buttonStyle="glass" aria-label="New" testId="new"
              image "symbol:add"
          column id="sort-menu" popover="auto" role="menu"
            button press=add popovertarget="sort-menu" popovertargetaction="hide"
              text "Date Added"
          text `${n}` testId="n"
```

- A `popovertarget` button is the bar item's pull-down menu (`contract vocab menu`). Write the
  menu beside the header; one written in it is still its button's menu, never more bar buttons.

More: docs/contract-for-agents.md, "Bar buttons, glass and the search field"

## haptics: a tap the hand feels
names: haptic, vibrate, vibration, Haptics, impactAsync, selectionAsync, notificationAsync, UIImpactFeedbackGenerator, UISelectionFeedbackGenerator, UINotificationFeedbackGenerator, sensoryFeedback, expo-haptics

`-exact-press-haptic` plays at touch-down on what is pressed, with no round trip:
`"selection"`, or `"impact-light"`, `-medium`, `-heavy`, `-soft`, `-rigid`. From an action,
`haptic(kind)` plays one of those or `"success"`, `"warning"` or `"error"`, as when a drag
crosses a threshold; an outcome is an action's, since a press has none at touch-down. A
native button (one with `buttonStyle`) plays its haptic from its action.
iOS uses the feedback generators, the web vibrates where it can, Linux does nothing.

```contract
component App
  state n = 0
  action add
    let next = n + 1
    n = next
    if next == 10
      haptic("success")
  view
    button press=add -exact-press-haptic="impact-light" testId="add"
      text `${n}`
```

## status-bar: the colour of the iPhone's status bar
names: status bar, statusbar, setStatusBarStyle, UIStatusBarStyle, preferredStatusBarStyle, expo-status-bar

`status-bar-style="light-content"` (light text, for a dark surface), `"dark-content"` or
`"auto"` on any node, bound to state if it changes, sets an iPhone's status bar; of what the
bar sits over, the declaration painted on top wins. `status-bar-animation="fade"` fades a
change. Other hosts ignore both.

More: docs/contract-for-agents.md (`status-bar-style`)

## toast: a message that shows for a moment
names: Toast, snackbar, Snackbar, react-native-toast-message, burnt

No host has a toast control, iOS included. A brief message is an overlay: a root child after
the page (after the `tablist` in a tab app) shown while state holds the message, and a
`task … when` that clears it with `after(ms, action)`.

```contract
component App
  state toast = ""
  action save
    toast = "Saved"
  action hide
    toast = ""
  task dismiss when toast != "" key=toast
    after(2000, hide)
  view
    column height="100%"
      button press=save testId="save"
        text "Save"
      if toast != ""
        text toast position="absolute" bottom=32 left=16 right=16 padding=12 border-radius=12 backgroundMaterial="thin" aria-live="polite" testId="toast"
```

- The action the task runs clears the message whatever it reads: a gated task is armed at
  commits, not by the clock (docs/agent-pitfalls.md, "A gate reads state at commits").

## titlebar: a row beside the Mac's traffic lights
names: titlebar-area, titlebar-area-x, titlebar-area-y, titlebar-area-width, titlebar-area-height, title-bar, traffic-lights, trafficLightPosition, window-controls-overlay, hiddenInset

Window Controls Overlay's `env(titlebar-area-x|y|width|height)` is the part of the title
bar the page may draw in, beside the window's controls. On the Mac, under
`viewport-fit="cover"`, it is AppKit's title bar less the traffic lights: in a 900-point
window `78 0 822×32`. It is all zero in full screen and while the window has a toolbar (an
app with routes has one), and undefined elsewhere: in a browser tab, on iOS, Linux and
Windows, and on a Mac window that does not cover its title bar. Give each a fallback.
`"display_override": ["window-controls-overlay"]` in `app.json` gives a Mac window the
platform's tall title row (an empty unified toolbar, about 52 points) while it shows no
toolbar of its own, whatever `viewport-fit` says, and the area is that row beside the
traffic lights; `exactViewport().displayMode` is then `window-controls-overlay`.

```contract
component App
  view
    main viewport-fit="cover" display="flex" flex-direction="column" width="100%" height="100%"
      row margin-left="env(titlebar-area-x, 0px)" width="env(titlebar-area-width, 100%)" height="env(titlebar-area-height, 0px)" align-items="center"
        text "Drafts" role="heading" aria-level=1
      scroll flex=1 min-height=0 padding=16
        text "Body"
```

- Nothing moves the traffic lights or sets the row's height: the title bar is AppKit's.
- More: docs/contract-grammar.md (lengths).

## share: the share sheet, with a link, words, a file or a picture of an element
names: share-sheet, sharesheet, UIActivityViewController, NSSharingServicePicker, navigator.share, Share.share, Sharing.shareAsync, shareAsync, share-card, share-image, share-png

`share(title=, text=, url=, files=)` opens the system share sheet with the Web Share API's
arguments. Each `text`, `url` and file is its own item: the sheet's header counts them ("1
Link and 1 Image") and every target receives all of them. Pass what the button names: a
Share Card button the picture alone, a Share Link button the `url`. A `files` entry `"#id"`
is a PNG of that element as drawn now, named by `title` (else by the id), so `title` names
the file a person receives; `title` is otherwise the subject Mail takes.

```contract
shape Page
  canSharePictures: bool

component App
  state name = "untitled"
  resource page = exactPage() as shape Page
  action shareCard
    share(title=`euxy-${name}`, files=["#card"])
  action shareLink
    share(url=`https://example.com/p/${name}`)
  view
    column padding=16 gap=12
      column id="card" padding=16 border-radius=12 background-color="#111111"
        text name color="#ffffff"
      if page.canSharePictures
        button press=shareCard testId="share-card"
          text "Share Card"
      button press=shareLink testId="share-link"
        text "Share Link"
```

- The PNG above is `euxy-untitled.png`, the one item in the sheet, as a React Native port's
  `Sharing.shareAsync(file)` shares it.
- A picture is iOS's and the Mac's; the web refuses one by name, so offer it where
  `exactPage().canSharePictures` holds. An `app:/` file needs the `fs.read` grant
  (`contract vocab files-and-grants`).
- More: docs/contract-grammar.md (host commands), LLP 1125 D12.

## use: an app in several `.contract` files
names: import, imports, export, exports, require, include, multiple files, several files, split

A file's names are its own declarations and those its `use` lines list: `use A, B as C from
"./file.contract"`. Any component, shape, `fn`, style, keyframes or timeline can be named;
there is no `export`, and nothing comes along unnamed: a name `parts.contract` itself uses is
this file's only once its own `use` line names it. `app.contract` stays the root, its first
component the app. A name another file declares is unknown here until a `use` line names it;
`bun exact.mjs contract fmt --uses app.contract` writes every file's missing `use` lines.

```text
// parts.contract
component Tag
  props
    label: string
  view
    text label padding="2px 8px"

// app.contract
use Tag from "./parts.contract"

component App
  view
    Tag(label="New")
```

More: docs/contract-grammar.md (`use`, its paths and packages)

## svg: a vector drawing (an icon, a chart, a QR code)
names: Svg, vector, react-native-svg, expo-svg, SvgXml, Path, Circle, Rect, drawing

An `svg` is SVG's, children and attributes by SVG's names: `viewBox`, `path d=…`, `rect`,
`circle`, `line`, `polyline`, `polygon`, `g transform=…`, `fill`, `stroke`, `stroke-width`,
`stroke-linecap`. Any attribute may be computed (a path's `d` from a `fn`, a `fill` from
state or a theme token), `if` and `for` work among its children, and keyframes animate its
geometry.

```contract
component App
  state on = false
  action flip
    on = not on
  view
    button press=flip aria-label=(on ? "Pause" : "Play") testId="play"
      svg width=24 height=24 viewBox="0 0 24 24"
        if on
          rect x=6 y=5 width=4 height=14 rx=1 fill="currentColor"
          rect x=14 y=5 width=4 height=14 rx=1 fill="currentColor"
        else
          path d="M8 5v14l11-7z" fill="currentColor"
```

- An icon the platform has is `image "symbol:<role>"`, drawn from each host's own set; a one-colour
  SVG file is an `app.tamber` icon (docs/recipes/symbols.md), and an `.svg` asset is an
  `image` source too.
- A `foreignObject` draws on the web only; on every host, place a box over the `svg`.

More: apps/svg-gallery/app.contract (each element and attribute, drawn alike on every host but `foreignObject`)

## safe-area: the notch, the status bar and the home indicator
names: env, env(), safe-area-inset, safe-area-inset-top, safe-area-inset-bottom, safe-area-inset-left, safe-area-inset-right, SafeAreaView, useSafeAreaInsets, react-native-safe-area-context, safeAreaInsets, notch, home indicator, home-indicator

Inside a route, leave it unsaid: the host keeps the route's content clear of the status bar
and its bars (UIKit's inset on the route's scroller, else the route's padding), and a
scroller's end clear of the home indicator, as a hand-built screen is. `env()` is CSS's, in
any length (`padding-bottom="calc(env(safe-area-inset-bottom) + 16px)"`). On a native host
under the default `viewport-fit="auto"` it is 0, so `env()` padding inside a route adds
nothing; under the root's `viewport-fit="cover"` it is the device's insets, and a route's
content goes under the bar unless its column pads `env(safe-area-inset-top)`. On the web it is
the browser's. `env(-exact-layout-margins-inset-left)` is UIKit's layout margin.

- A scroller's last row sits its `padding-bottom` above UIKit's inset: subtract the inset
  from the original's gap rather than adding `env(safe-area-inset-bottom)`.

More: docs/agent-pitfalls.md ("Native presentation and navigation (iOS)"), docs/contract-grammar.md (length strings)

## var: a design token by CSS's `var(--name)`, and a stylesheet's custom properties
names: var(), custom-property, custom-properties, tokens.css

`var(--name)` in a style row is the token `name` of the app's design (`app.tamber`, or a root
`theme`), and compiles exactly as writing `name` does: folded, or on the web `var(--x-name)`
where the token varies by an app mode. It is the build's reference, not the browser's: no
stylesheet redefines it, and every host draws it. A name the design lacks is refused, naming
the tokens the row could take, whether or not a fallback is written, so a misspelt name never
ships the fallback; a declared name's fallback is ignored and warned (`tamber-var-fallback`).

```contract
theme
  ink = "light-dark(#1d1d1f, #f5f5f7)"
  card = "light-dark(#f2f2f7, #1c1c1e)"

component App
  view
    column background-color="var(--card)" padding=16 gap=8
      text "Tonight" color="var(--ink)"
      text "3 patterns" color="ink"
```

- `bun exact.mjs tamber import css tokens.css` writes a stylesheet's `:root` custom properties
  into `app.tamber`, a `prefers-color-scheme: dark` redeclaration as `light-dark()`, between
  `// tamber:import tokens.css` and `// tamber:end`; after an edit, import again. A `.dark` or
  `[data-theme]` scope, or a value built around a `var()`, is refused by name.
- A token of another kind (`color="var(--space)"` for a length) is refused as its name is.
- JavaScript's `var`, a name inside an action, is `let` (`contract vocab let`).

More: docs/recipes/tamber.md, docs/recipes/theme.md
