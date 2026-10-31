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
Look up: `contract vocab <name> …`; list: `--all`.

## functions
at: xs.at(i) -> option<T>  (-1 is last)
calendarDiff: calendarDiff(from, to, "years"|"months") -> option<number>  (whole; YYYY-MM-DD)
concat: xs.concat(ys) -> T[]
filter: xs.filter(x => bool) -> T[]
flat: xs.flat() -> T[]
first: first(xs) -> option<T>
formatDate: formatDate(ms, offsetMinutes, "medium"|"month-year"|"iso") -> string
formatDay: formatDay(then, now, offsetMinutes, "day"|"day-time"|"list") -> string  (Today, Monday)
formatDecimal: (cents to "12.34"; literal digits)
formatNumber: formatNumber(n, "compact"|"decimal"|"percent"|"currency", "USD") -> string  (1,234.5; 26%)
formatTime: ("2:13 PM"; minutes east of UTC)
frame: (action)
includes: s.includes(x) -> bool  (text or list)
indexOf: s.indexOf(x) -> number  (-1 for none)
isEmpty: s.isEmpty() -> bool
join: xs.join(sep) -> string
length: s.length -> number  (text or list)
map: xs.map(x => U) -> U[]
measure: (action)
parseNumber: (none for "12px")
performanceNow: (ms since launch)
rangeFrame: (action)
round: (half up)
slice: s.slice(start, end?) -> text|list
sortBy: sortBy(xs, x => key, "desc"?) -> T[]  (stable)
t: t("key", name=value) -> string  (localized)
toFixed: (literal digits)
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
time | datetime format
progress | value max start end
list | listStyle appearance virtualized refreshing scroll-start | reachstart reachend refresh
scroll | scrollTop navigationScroll keyboardDismissMode | scroll scrollend
dialog | closedby | cancel close

## rules
- An event hands its action its payload after the captured arguments: text for a field, `select` or date; a bool for a checkbox or switch; a number for a range; nothing for `press` and `submit`. `change=set(i.id)` calls `set(id, payload)`.

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
- A number field's `min`, `max`, `step` are numbers (`min=-40`); with `inputmode="decimal"` a negative `min` gives the pad its ± key.
- Symbols: `"symbol:<role>"` (below); `symbol:sf/<name>` is Apple-only.
- Not in Contract (`contract vocab <name>` says what instead): abs pow sqrt sort reduce find some every contains padStart parseInt.

## how
### a second screen, a sheet
A screen is a route, not an `if`; a sheet's root adds `navigationPresentation="modal"`, its Cancel the `id` `navigationBack` names (`pattern list-detail`).
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
          button id="back" press=back aria-label="Cancel"
            image "symbol:close"
          text "New" role="heading"
          button press=save buttonStyle="bordered-prominent" aria-label="Save"
            image "symbol:checkmark"
```
### a confirmation
What can't be undone (Delete) asks first. `aria-label` is the question; actions run their `press` and close (`command="close"`); Cancel has no `press`. Open it with `command="show-modal"` or `showModal(id)`.
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
`every(ms, action)` repeats; `after(ms, action)` runs once. Options: `wait=frame|update|scrollend|idle`, `max=ms`, `tolerance=ms`. See `contract vocab timers`.
```contract
state now = 0
task tick mount
  every(1000, step)
action step
  now = performanceNow()
text `${now}`
```
### keeping a value
Root `state` only: a number, string or bool (or an option or list of one) up to 16 KB. Records belong in a source (`files-and-grants`).
```contract
state units = "C" persist
```
### colour
Leave the accent unsaid (a brand is an app.tamber) and controls unpainted; elsewhere a role (follows the scheme): text `label`, `secondary-label`; a card `secondary-background`; a fill `secondary-fill`; a hue `system-red`…. No hex.
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
The write reads the clock, not a ticked `now`, and stores `at`; `performanceNow()` alone is 0 each launch. `"iso"` is the day, compared as text; its label `formatDay(at, now, …)`: Today, Yesterday. Tests pin `epoch` and `time-zone`.
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
Each test starts with empty storage; target by `testId` or text. `clock data` lands replies, `clock settle` settles presentation, `clock held` lands held replies; `reload` keeps storage. `bun exact.mjs verify` tests, checks end screens and relaunches; `verify ios` last; `--shots <dir>` keeps screenshots.
It runs `contract verify` too; keep its receipt.
```contract-test
test "a pick survives a relaunch"
  tap "pick"
  clock data
  reload
  expect text "n" == "1"
```
### composing a screen
One focal point. Emphasis is a part, not paint on a `button`; a longer main action is `buttonStyle="filled"`. Search: `input type="search"` in the `header`, filtered live. A form is a grouped list (`grouped-form`); no rows, an `EmptyState`, its `button` indented under it.
```contract
use SummaryCard from "exact:content"
column gap=16 padding=16
  SummaryCard(label="Streak", value="12", detail="days in a row", progress=none, symbol=none)
  button buttonStyle="filled"
    text "Log today"
```
