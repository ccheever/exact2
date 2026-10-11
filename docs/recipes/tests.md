# Recipe: tests

`app.test.contract` drives the app by `testId`, the same on the web and iOS (`bun
exact.mjs test web`, `test ios`). Each test is a fresh launch with empty storage, after
its first data has landed. Against [start-here](../start-here.md)'s Groceries:

```contract-test
test "a repeat is refused and keeps its text; the list survives a relaunch"
  type "draft" "Eggs"
  type "draft" key "Enter"
  clock data
  type "draft" "eggs"
  tap "add"
  clock data
  expect text "add-error" == "Eggs is already on the list."
  expect state draft == "eggs"
  reload
  expect text "left" == "1 item left"
```

| Step | What it does |
| --- | --- |
| `tap "id"` (`… contextmenu`) | Presses (long-presses) it; `tap "Milk"`: by text or label. |
| `type "id" "text"`, `type "id" key "Enter"` | Sets a value (a `select`'s, a date `"2026-10-10"`, a checkbox `"true"`); presses a key. |
| `expect text "id" == "…"` | Its text, else its value, else its descendants' text. |
| `expect tree has "id"` (`missing`) · `expect state groceries.0 .title == "Eggs"` | In the tree or not · a state, derive or resource field. |
| `clock data` · `clock settle` · `clock +5000` · `reload` | Lands replies · ends transitions · moves the virtual clock · relaunches on the same storage. |
| `fail fetch "<prefix>"`, `pass fetch …` | A lost connection there, then none. |
| `size 390x844`, `epoch`, `time-zone`, `locale`, `seed`, `before data` | Launch lines: before a test's steps, or atop the file. |

Every other form (`screenshot`, `resize`, drags): [the grammar](../contract-grammar.md#authored-tests).
An `expect` has no `<` or `contains`: compare a derive with `==`. `clock data` after an
input whose reply the next `expect` reads (iOS needs it); `clock settle` after a sheet, an
alert or a push. Test the refusal as well as the success; never wait in real time in a
test (pin `epoch` and `time-zone`); give every screen its own ids.

## Before you call it done

Tests pass while a real write hangs or a layout spills off the screen. Drive each write
once on the real clock:

```sh
bun exact.mjs agent web --storage real1 --epoch now "type draft Milk" "tap add" "clock +1500 real" state
bun exact.mjs agent web --storage real1 --epoch now "clock +1500 real" state "screenshot .exact/real-web.png"
```

The second drive on the same `--storage` is the relaunch: `reload` is a test line, not a
drive's op; `state` lists what is `pending` or `failed`. iOS: `agent ios --chrome
platform`, `"screenshot … window"` ([ios-driving](ios-driving.md)). One drive in dark
mode: `"prefer prefers-color-scheme dark"` first.
