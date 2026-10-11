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
  expect state groceries.items.0 .done == true
  expect state left == 0
  reload
  expect text "left" == "0 items left"
  screenshot ".exact/after-reload.png"

test "Enter in the field adds too"
  type "draft" "Eggs"
  type "draft" key "Enter"
  clock data
  expect state groceries.items.0 .title == "Eggs"
  expect state draft == ""
```

| Step | What it does |
| --- | --- |
| `tap "id"` | Presses it. `tap "Milk"` names a node by its exact text or label; `tap "id" contextmenu` long-presses. |
| `type "id" "text"` | Sets a field's value; a `select`'s value, a date (`"2026-10-10"`), a checkbox (`"true"`). `type "id" key "Enter"` presses a key. |
| `expect text "id" == "…"` | The node's text, else a control's value, else its descendants' text. |
| `expect tree has "id"`, `missing` | Whether the node is in the tree (covered routes stay in it). |
| `expect state a.b == v` | A state, derive or resource field: a number, a string, `true`, `false`, `none` or `[]`. A list index is `items.0 .title` (a space before the field). |
| `clock data` | Lands storage and fetch replies; the clock stays still. |
| `clock settle` | Runs transitions (a push, a sheet, an alert) to their end. |
| `clock +5000` | Moves the virtual clock 5 s: timers fire, animations advance. |
| `reload` | Relaunches on the same storage; the clock starts over at 0. |
| `fail fetch "<prefix>"`, `pass fetch "<prefix>"` | Fetches to that prefix fail as a lost connection, then stop failing. First in a test, it is armed before the first load. |
| `size 390x844`, `epoch "2026-03-14T09:00:00Z"`, `time-zone "Europe/Paris"`, `locale "fr-FR"`, `seed 7` | Launch lines: before a test's steps, or at the top for every test. |

An `expect` has no `<`, `contains` or `length`: give the app a derive (`derive left =
length(…)`) and compare it with `==`.

- `clock data` after any input whose reply the next `expect` reads (the web often
  answers in time without it, iOS doesn't). `clock settle` after opening or closing a
  sheet, an alert or a push, and before tapping inside an alert.
- Never wait in real time (`clock +1000 real`) in a test. A test that reads the date
  pins `epoch` and `time-zone`; the driver's own are 2026-01-01T00:00:00Z, UTC, seed 1.
- Give every screen its own ids: covered routes stay mounted, so a shared `testId`
  names two nodes.

## Before you call it done

Tests run on a virtual clock, and pass while a real write hangs or a layout spills off
the screen. Drive each write once on the real clock, in bounded steps, and look:

```sh
bun exact.mjs agent web --storage real1 --epoch now "type draft Milk" "tap add" "clock +1500 real" \
  state "reload" "clock +1500 real" state "screenshot .exact/real-web.png"
bun exact.mjs agent ios --storage real1 --epoch now --chrome platform "type draft Milk" "tap add" \
  "clock +1500 real" "reload" "clock +1500 real" state "screenshot .exact/real-ios.png window"
```

Each `clock +N real` waits N ms of real time, so the drive ends. `state` lists what is
`pending` or `failed`; the iOS screenshot with `--chrome platform … window` shows the
bars a person sees. An app with a server drives it against the running dev server.
