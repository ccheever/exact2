Exact card. Look up: `contract vocab <name> …`; list: `--all`.

functions: call -> result
  xs.at(i) -> option<T>  (none past end; -1 is last)
  back(router) -> Router
  calendarDiff(from, to, "years" or "months") -> option<number>  (whole ones; YYYY-MM-DD dates)
  ceil(n) -> number
  xs.concat(ys) -> T[]
  depth(router) -> number
  elementFromPoint(x, y) -> option<string>
  elementsFromPoint(x, y) -> string[]
  encodeRouteSegment(text) -> string
  encodeURIComponent(text) -> string
  text.endsWith(suffix) -> bool
  xs.filter(x => bool) -> T[]
  first(xs) -> option<T>
  floor(n) -> number
  formatDate(ms, offsetMinutes, "medium"|"month-year"|"iso") -> string
  formatDay(then, now, offsetMinutes, "day"|"day-time"|"list") -> string  (Today, Yesterday, Monday)
  formatDecimal(units, digits) -> string  (cents to "12.34"; digits a literal)
  formatNumber(n, "compact"|"decimal"|"percent"|"currency", "USD") -> string  (1,234.5; 26%)
  formatTime(ms, offsetMinutes, "short") -> string  ("2:13 PM"; minutes east of UTC)
  frame(id) -> Geometry  (in an action)
  go(router, location) -> Router
  s.includes(x) -> bool  (text or list)
  s.indexOf(x) -> number  (-1 for none)
  s.isEmpty() -> bool
  xs.join(sep) -> string
  s.length -> number  (text or list)
  xs.map(x => U) -> U[]
  max(a, b) -> number
  measure("id") -> Geometry  (in an action)
  min(a, b) -> number
  open(router, location) -> Router
  params(router, parameterName) -> string[]
  parseNumber(text) -> option<number>  (none for "", "12px", "1.2.3")
  performanceNow() -> number  (ms since launch)
  push(router, location) -> Router
  querySelectorAll(selector) -> string[]
  replace(router, location) -> Router
  text.replaceAll(find, with) -> string
  round(n) -> number  (half up)
  searchParam(entry, name) -> string
  select(router, tabName) -> Router
  s.slice(start, end?) -> text|list
  text.split(separator) -> string[]
  stack(router) -> Entry[]
  text.startsWith(prefix) -> bool
  t("key", name=value) -> string  (the locale's table)
  n.toFixed(digits) -> string  (digits a literal, 0 to 100)
  text.toLowerCase() -> string
  top(router) -> Entry
  x.toString() -> string
  text.trim() -> string
  pending(resourceOrMutationName)  failed(resourceOrMutationName)  failure(resourceOrMutationName)  path("routeName", args…)  empty(field=constant, …)  some(expr)  DeclaredShape(field=value, …)

tags: attributes; on events. Every tag takes CSS's style attributes (flex gap padding font-size …) and colour roles (-exact-label); `contract vocab <name>` gives a style's values
  view box row column main nav article aside section header footer hr: id testId role title tabindex; on press hover key keyup focus blur contextmenu dblclick
  text "…": markup; on selectionchange
  image "…": alt; on load
  button: disabled buttonStyle destructive; on press
  link: href; on press
  input: value type inputmode min max step maxlength placeholder enterkeyhint autocomplete autocapitalize autofocus disabled readonly checked name accept multiple; on input change submit select selectionchange beforeinput
  textarea: value placeholder maxlength enterkeyhint; on input change select selectionchange beforeinput
  select: value disabled; on change
  option "…": value
  progress: value max
  list: listStyle appearance virtualized refreshing scroll-start; on reachstart reachend refresh
  scroll: scrollTop navigationScroll keyboardDismissMode; on scroll scrollend
  dialog: closedby; on cancel close

parts: use X from "exact:content"; an option is none or some(…)
  SummaryCard(label="Streak", value="12", detail="days in a row", progress=some(0.4), symbol=some("symbol:star"))
  StatBar(label="Protein", value="82 g", fraction=0.7, tint=none)
  RingProgress(label="Steps", value="6,200", fraction=0.62, tint=none)
  DoneMark(caption="Mon", done=true)
  EmptyState(symbol="symbol:sparkles", title="No habits yet", message="Add one to start a streak.")
  RowText(title="Oat milk", detail="2 in stock", value=some("$3.50"))
  Badge(label="Full", tint=some("-exact-system-orange"))
  RoundButton(label="Start", tint=some("-exact-system-green"), press=toggle, disabled=false, testId="start")
  AnswerTile(label="Paris", status="idle", press=pick("Paris"), testId="answer-paris")

test steps (app.test.contract; every other form: docs/contract-grammar.md)
  tap "id" (… contextmenu)
  type "id" "text", type "id" key "Enter"
  expect text "id" == "…"
  expect tree has "id" (missing) · expect disabled "id" (enabled) · expect state groceries.0 .title == "Eggs"
  clock data · clock settle · clock +5000 · reload
  back
  fail fetch "<prefix>", pass fetch …
  request POST "/items" {…} as grace (expect 409)
  size 390x844, epoch, time-zone, locale, seed, before data

ops: a target is a testId, else a view's exact text or label
  tree [<target>] · tree --ax · logs · layout [<target>]
  state [<name>]  a view's testId, else a state or resource, dotted: groceries.0.done
  tap <target> [contextmenu|dblclick|hover|drag <dx> <dy>|…]
  type <target> <text> · type <target> key <Name>   (a select, date or checkbox: its value)
  clock data · clock settle · clock +<ms> [real]    land what is in flight · end transitions
  check [dark|both] · screenshot <file.png> [window]  what is wrong, in text, before a picture · window: bars
  prefer <feature> <value> · perf [<target>] · resize <w>x<h> · close · back · fail|pass fetch <prefix>

how (snippets compile)
a second screen, a sheet: A screen is a route, never an `if` on state; a sheet's root adds `navigationPresentation="modal"`, its Cancel the `id` `navigationBack` names. `pattern list-detail` is the whole.
  routes nav
    home "/"
      edit "/edit"
  action open
    nav = push(nav, "/edit")
  action back
    nav = back(nav)
  action save
    nav = back(nav)
  main navigationKey=`${top(nav).id}` navigationBack="back"
    for e in stack(nav) key=e.id
      if e.name == "home"
        column navigationKey=`${e.id}` position="absolute" inset=0
          header
            text "Notes" role="heading" aria-level=1
            button press=open
              text "New"
      if e.name == "edit"
        column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0
          header
            button id="back" press=back
              text "Cancel"
            text "New" role="heading"
            button press=save
              text "Save"
a confirmation: `aria-label` is the question; each action `button` acts and closes (`command="close"`); the Cancel has no `press`. Open it with `command="show-modal"`, or `showModal(id)` in an action.
  state n = 1
  action remove
    n = 0
  button commandfor="remove" command="show-modal"
    text "Remove"
  dialog id="remove" role="alertdialog" aria-label="Delete it?"
    button press=remove destructive=true commandfor="remove" command="close"
      text "Delete"
    button commandfor="remove" command="close"
      text "Cancel"
a timer: `every(ms, action)` repeats, `after(ms, action)` fires once, `when cond` arms the task. A `derive` never rereads the clock: an action writes `performanceNow()` to state.
  state now = 0
  task tick mount
    every(1000, step)
  action step
    now = performanceNow()
  text `${now}`
keeping a value: Root `state` only: a number, string or bool (or an option or list of one) up to 16 KB, read before the first frame. Records belong in a source (`files-and-grants`).
  state units = "C" persist
colour: Leave the accent unsaid (the platform's blue; a brand is an app.tamber) and controls unpainted; elsewhere a role, which follows the scheme: text `label`, `secondary-label`; a card `secondary-background`; a fill `secondary-fill`; a hue `system-red`…. No hex.
  main
    text "Total" color="-exact-secondary-label"
    view padding=12 background-color="-exact-secondary-background"
      text "Over" color="-exact-system-red"
reading and writing data: `loadItems` and `addItem` are in app.ts and answer exactly their shape; a refusal is a reply (`ok: false, error`), not a throw. `then act` after `refreshes` clears the field on an ok reply; `pending(added)` stops a second send.
  shape Reply
    ok: bool
  resource items = loadItems() as shape string[] else empty()
  mutation added as shape Reply refreshes items
  action add
    send added = addItem("Milk")
  text items.join(", ")
a dated record: The write reads the clock itself, not a ticked `now`, and stores `at`, never `performanceNow()` alone (0 at each launch). `"iso"` is the day, compared as text. Tests pin `epoch` and `time-zone`.
  shape Time
    epochAtZero: number
    utcOffset: number
  state days = []
  resource time = exactTime() as shape Time
  action log
    let at = time.epochAtZero + performanceNow()
    days = days.concat([formatDate(at, time.utcOffset, "iso")])
a test, and verify: By `testId` or exact text, each test from empty storage. `clock data` after a reply, `clock settle` after a push, sheet or alert; `reload` relaunches on the same storage; never wait in real time. `bun exact.mjs verify` runs the tests, checks each screen a test ends on and relaunches once; `verify ios` once at the end for an iPhone app; `--shots <dir>` keeps the screenshots. A screen no test ends on is unchecked.
  test "a pick survives a relaunch"
    tap "pick"
    clock data
    reload
    expect text "n" == "1"
composing a screen: Chrome is the platform's; the content is yours, with one focal point. Emphasis is a part (above), not paint on a `button`; a longer main action is `buttonStyle="filled"`. Search is `input type="search"` in the `header`, the bar's field, filtering as you type. An entry form is a grouped list (`grouped-form`); no rows, an `EmptyState` in the list's place.
  use SummaryCard from "exact:content"
  column gap=16 padding=16
    SummaryCard(label="Streak", value="12", detail="days in a row", progress=none, symbol=none)
    button buttonStyle="filled"
      text "Log today"

rules
- An event hands its action its payload after the captured arguments: the text for a field, a `select` or a date; the bool for a checkbox or switch; the number for a range; nothing for `press` and `submit`. `change=set(i.id)` calls `set(id, payload)`.

```contract
component App
  state name = ""
  state on = false
  action setName(v: string)
    name = v
  action setOn(b: bool)
    on = b
  view
    column
      input value=name input=setName inputmode="decimal" min=-40
      input type="checkbox" checked=on change=setOn
```
- `select value=v change=set` holds `option "Label" value="x"`. Tabs: a `row role="tablist"` of `button role="tab" aria-controls="panel-x" aria-selected=… press=…`, each panel a `column role="tabpanel" id="panel-x"`; `routes nav`, `tab`, `select(nav, name)`.
- A number field's `min`, `max`, `step` take numbers, negative bare (`min=-40`); with `inputmode="decimal"` a negative `min` gives the pad its ± key.
- A symbol is `"symbol:<role>"` (roles below); `symbol:sf/<name>` is Apple-only.
- Not in Contract, each with what to write in `contract vocab <name>`: abs pow sqrt sort reduce find some every contains toUpperCase padStart parseInt.
- `a - b` subtracts with spaces; a unit is quoted (`width="50%"`); a view repeats with `for x in xs key=…`.
idioms, whole with their code (`contract vocab <name>`): scroll loading plural choice screen notfound modal confirm date persist swipe task mutation resource match for if let fn shape props derive action stepper gradients menu search menubar ::placeholder
symbol roles: back close compose add microphone send search copy select-text more delete forward home person messages notifications settings repeat activity bookmark bookmark-fill document select select-fill reorder sort filter heart heart-fill reply share ellipsis home-fill notifications-fill person-fill photo play play-fill pause pause-fill skip-back-15 skip-forward-15 skip-back-30 skip-forward-30 speaker speaker-mute moon pin link checkmark forward-chevron at hashtag follow sign-out quote feeds calendar book layers lock people stop stop-fill circle alert minus-circle pencil waveform info camera star star-fill minus chart sparkles refresh menu adjust alert-fill info-fill chooser download install open-app download-pending package mail
