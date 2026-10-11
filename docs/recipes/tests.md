# Recipe: tests

`app.test.contract` drives the app by `testId`, the same on the web and iOS (`bun
exact.mjs test web`, `test ios`). Each test is a fresh launch with empty storage, after
its first data has landed. These run against [start-here](../start-here.md)'s
Groceries app:

```contract-test
size 390x844

test "add, check off, and keep it across a relaunch"
  expect tree missing "done-1"
  type "draft" "Milk"
  tap "add"
  clock data
  expect tree has "done-1"
  expect text "left" == "1 item left"
  tap "done-1"
  clock data
  expect state groceries.0 .done == true
  expect state left == 0
  reload
  expect text "left" == "0 items left"
  screenshot ".exact/after-reload.png"

test "Enter adds too; a repeat is refused and keeps its text"
  type "draft" "Eggs"
  type "draft" key "Enter"
  clock data
  expect state groceries.0 .title == "Eggs"
  expect state draft == ""
  type "draft" "eggs"
  type "draft" key "Enter"
  clock data
  expect text "add-error" == "Eggs is already on the list."
  expect state draft == "eggs"
```

| Step | What it does |
| --- | --- |
| `tap "id"` | Presses it. `tap "Milk"` names a node by its exact text or label; `tap "id" contextmenu` long-presses. |
| `type "id" "text"` | Sets a field's value; a `select`'s value, a date (`"2026-10-10"`), a checkbox (`"true"`). `… append` adds to the value; `type "id" key "Enter"` presses a key. |
| `expect text "id" == "…"` | The node's text, else a control's value, else its descendants' text. |
| `expect tree has "id"`, `missing` | Whether the node is in the tree (covered routes stay in it). |
| `expect state a.b == v` | A state, derive or resource field: a number, a string, `true`, `false`, `none` or `[]`. A list index is `items.0 .title` (a space before the field). |
| `clock data` | Lands storage and fetch replies; the clock stays still. |
| `clock settle` | Runs transitions (a push, a sheet, an alert) to their end. |
| `clock +5000` | Moves the virtual clock 5 s: timers fire, animations advance. |
| `reload` | Relaunches on the same storage; the clock starts over at 0. |
| `fail fetch "<prefix>"`, `pass fetch "<prefix>"` | Fetches to that prefix fail as a lost connection (`times 2`: only the next two), then stop failing. First in a test, it is armed before the first load. |
| `screenshot ".exact/x.png"`, `resize 800x600` | A picture mid-test, to look at; the window resized mid-test. |
| `size 390x844`, `epoch "2026-03-14T09:00:00Z"`, `time-zone "Europe/Paris"`, `locale "fr-FR"`, `seed 7` | Launch lines: before a test's steps, or at the top for every test. |
| `before data` | A launch line: the first step runs before the first data lands (a loading state). |

Every other form (drags, pinches, `pick`, `close`, sounds): [the grammar](../contract-grammar.md#authored-tests).

An `expect` has no `<`, `contains` or `length`: give the app a derive (`derive left =
length(…)`) and compare it with `==`.

- `clock data` after any input whose reply the next `expect` reads (the web often
  answers in time without it, iOS doesn't). `clock settle` after opening or closing a
  sheet, an alert or a push, and before tapping inside an alert.
- Test the refusal as well as the success: drive the write that is refused, `clock
  data`, then expect the reason and the text kept. A lost connection is `fail fetch
  "<prefix>"` before the write ([rest-backend](rest-backend.md)).
- Never wait in real time (`clock +1000 real`) in a test. A test that reads the date
  pins `epoch` and `time-zone`; the driver's own are 2026-01-01T00:00:00Z, UTC, seed 1.
- Give every screen its own ids: covered routes stay mounted, so a shared `testId`
  names two nodes.

## Before you call it done

Tests run on a virtual clock, and pass while a real write hangs or a layout spills off
the screen. Drive each write once on the real clock, in bounded steps, and look:

```sh
bun exact.mjs agent web --storage real1 --epoch now "type draft Milk" "tap add" "clock +1500 real" state
bun exact.mjs agent web --storage real1 --epoch now "clock +1500 real" state "screenshot .exact/real-web.png"
bun exact.mjs agent ios --storage real1 --epoch now --chrome platform "type draft Milk" "tap add" \
  "clock +1500 real" state
bun exact.mjs agent ios --storage real1 --epoch now --chrome platform "clock +1500 real" state \
  "screenshot .exact/real-ios.png window"
```

The second drive on the same `--storage` is the relaunch: `reload` is a test line, not
a drive's op. Each `clock +N real` waits N ms of real time, so the drive ends. `state` lists what is
`pending` or `failed`; the iOS screenshot with `--chrome platform … window` shows the
bars a person sees ([ios-driving](ios-driving.md)). An app with a server drives it
against the running dev server. A test has no colour scheme, so look at one drive in
dark mode too: `"prefer prefers-color-scheme dark"` as its first op.
