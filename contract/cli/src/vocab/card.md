<!--
`contract vocab` with no name prints the card (LLP 1116.010 D1): what a builder looks up one call at a
time, in one read, at most 12 KB (`contract/cli/tests/it/vocab.rs` holds it). `docs/card.md` is the same
text checked in for builders that read files (LLP 1116.011 D1; a test keeps the two identical). `card.rs` generates it from
the tables: the functions from the roster and the grammar, the parts from `exact:content`'s calls, the
steps from docs/recipes/tests.md, the agent's operations from scripts/help.mjs, the symbols from the
schema. This file holds what no table says, and a test checks it against them: every tag, attribute and
event named below exists, every function it rewrites exists, every `contract` block compiles.

A section is `## name`. `functions`: `name: text` replaces that function's generated line, `name: (text)`
adds words to it. `tags`: `tag tag | attribute attribute | event event`, checked word by word. `how`: a
`### title`, its rule, one fence: `contract` lines of an app without its frame (a test puts the file-level lines,
the component's and the view's in place and compiles it) or `contract-test` (parsed); the `--flags` a rule names
are checked against scripts/help.mjs.
-->

## intro
Exact card. Look up: `contract vocab <name> …`; list: `--all`.

## functions
at: xs.at(i) -> option<T>  (none past end; -1 is last)
calendarDiff: (whole ones; YYYY-MM-DD dates)
concat: xs.concat(ys) -> T[]
filter: xs.filter(x => bool) -> T[]
first: first(xs) -> option<T>
formatDate: formatDate(ms, offsetMinutes, "medium"|"month-year"|"iso") -> string
formatDay: formatDay(then, now, offsetMinutes, "day"|"day-time"|"list") -> string  (Today, Yesterday, Monday)
formatDecimal: (cents to "12.34"; digits a literal)
formatNumber: formatNumber(n, "compact"|"decimal"|"percent"|"currency", "USD") -> string  (1,234.5; 26%)
formatTime: ("2:13 PM"; minutes east of UTC)
frame: (in an action)
includes: s.includes(x) -> bool  (text or list)
indexOf: s.indexOf(x) -> number  (-1 for none)
isEmpty: s.isEmpty() -> bool
join: xs.join(sep) -> string
length: s.length -> number  (text or list)
map: xs.map(x => U) -> U[]
measure: (in an action)
parseNumber: (none for "", "12px", "1.2.3")
performanceNow: (ms since launch)
round: (half up)
slice: s.slice(start, end?) -> text|list
t: t("key", name=value) -> string  (the locale's table)
toFixed: (digits a literal, 0 to 100)
toString: x.toString() -> string

## tags
view box row column main nav article aside section header footer hr | id testId role title tabindex | press hover key keyup focus blur contextmenu dblclick
text | markup | selectionchange
image | alt | load
button | disabled buttonStyle destructive | press
link | href | press
input | value type inputmode min max step maxlength placeholder enterkeyhint autocomplete autocapitalize autofocus disabled readonly checked name accept multiple | input change submit select selectionchange beforeinput
textarea | value placeholder maxlength enterkeyhint | input change select selectionchange beforeinput
select | value disabled | change
option | value
progress | value max
list | listStyle appearance virtualized refreshing scroll-start | reachstart reachend refresh
scroll | scrollTop navigationScroll keyboardDismissMode | scroll scrollend
dialog | closedby | cancel close

## rules
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

## how
### a second screen, a sheet
A screen is a route, never an `if` on state; a sheet's root adds `navigationPresentation="modal"`, its Cancel the `id` `navigationBack` names. `pattern list-detail` is the whole.
```contract
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
```
### a confirmation
`aria-label` is the question; each action `button` acts and closes (`command="close"`); the Cancel has no `press`. Open it with `command="show-modal"`, or `showModal(id)` in an action.
```contract
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
```
### a timer
`every(ms, action)` repeats, `after(ms, action)` fires once, `when cond` arms the task. A `derive` never rereads the clock: an action writes `performanceNow()` to state.
```contract
state now = 0
task tick mount
  every(1000, step)
action step
  now = performanceNow()
text `${now}`
```
### keeping a value
Root `state` only: a number, string or bool (or an option or list of one) up to 16 KB, read before the first frame. Records belong in a source (`files-and-grants`).
```contract
state units = "C" persist
```
### colour
Leave the accent unsaid (the platform's blue; a brand is an app.tamber) and controls unpainted; elsewhere a role, which follows the scheme: text `label`, `secondary-label`; a card `secondary-background`; a fill `secondary-fill`; a hue `system-red`…. No hex.
```contract
main
  text "Total" color="-exact-secondary-label"
  view padding=12 background-color="-exact-secondary-background"
    text "Over" color="-exact-system-red"
```
### reading and writing data
`loadItems` and `addItem` are in app.ts and answer exactly their shape; a refusal is a reply (`ok: false, error`), not a throw. `then act` after `refreshes` clears the field on an ok reply; `pending(added)` stops a second send.
```contract
shape Reply
  ok: bool
resource items = loadItems() as shape string[] else empty()
mutation added as shape Reply refreshes items
action add
  send added = addItem("Milk")
text items.join(", ")
```
### a dated record
The write reads the clock itself, not a ticked `now`, and stores `at`, never `performanceNow()` alone (0 at each launch). `"iso"` is the day, compared as text. Tests pin `epoch` and `time-zone`.
```contract
shape Time
  epochAtZero: number
  utcOffset: number
state days = []
resource time = exactTime() as shape Time
action log
  let at = time.epochAtZero + performanceNow()
  days = days.concat([formatDate(at, time.utcOffset, "iso")])
```
### a test, and verify
By `testId` or exact text, each test from empty storage. `clock data` after a reply, `clock settle` after a push, sheet or alert; `reload` relaunches on the same storage; never wait in real time. `bun exact.mjs verify` runs the tests, checks each screen a test ends on and relaunches once; `verify ios` once at the end for an iPhone app; `--shots <dir>` keeps the screenshots. A screen no test ends on is unchecked.
```contract-test
test "a pick survives a relaunch"
  tap "pick"
  clock data
  reload
  expect text "n" == "1"
```
### composing a screen
Chrome is the platform's; the content is yours, with one focal point. Emphasis is a part (above), not paint on a `button`; a longer main action is `buttonStyle="filled"`. Search is `input type="search"` in the `header`, the bar's field, filtering as you type. An entry form is a grouped list (`grouped-form`); no rows, an `EmptyState` in the list's place.
```contract
use SummaryCard from "exact:content"
column gap=16 padding=16
  SummaryCard(label="Streak", value="12", detail="days in a row", progress=none, symbol=none)
  button buttonStyle="filled"
    text "Log today"
```
