# Recipe: tests

`app.test.contract` drives the app by `testId`, the same on the web and iOS (`bun
exact.mjs test web`, `test ios`; `verify` and `verify ios` run them, then `check`). Each
test is a fresh launch with empty storage, after its first data has landed. Against
[start-here](../start-here.md)'s Groceries:

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
| `back` | The platform's Back: cancels a dialog, else dismisses a sheet, else pops the pushed route; fails with nothing to go back from. |
| `fail fetch "<prefix>"`, `pass fetch …` | A lost connection there, then none. |
| `request POST "/items" {"title": "Bread"} as grace` | The driver writes through the app's server as another person ([rest-backend](rest-backend.md)); `clock +<poll ms>`, `clock data`, then `expect`. |
| `size 390x844`, `epoch`, `time-zone`, `locale`, `seed`, `before data` | Launch lines: before a test's steps, or atop the file. |

Every other form (`screenshot`, `resize`, drags): [the grammar](../contract-grammar.md#authored-tests).
An `expect` has no `<` or `contains`: compare a derive with `==`. `clock data` after an
input whose reply the next `expect` reads (iOS needs it); `clock settle` after a sheet, an
alert or a push. Test the refusal as well as the success; never wait in real time in a
test (pin `epoch` and `time-zone`); give every screen its own ids.

## Before you call it done

`verify` (`verify ios`) runs these tests, then `check`s the first screen and the screen
each test ends on, light and dark at a phone and a desktop width: clipped text, covered
controls, small targets, overflow, low contrast, a card that does not show. Then it
relaunches once, on real time, on the store a passing test left, and says whether the
first screen shows what that test stored. One screenshot of each screen is in
`.exact/verify/`, named by test. A clean `verify` is done: look at the screenshots at
milestones. A screen no test ends on is not checked, so end a test there rather than
driving it by hand. Drive by hand (`agent web --storage s1 …`; `"clock +1500 real"` waits
real time, and a second drive on the same `--storage` relaunches) to find out why
something fails.

A second user's write, seen through the app's own polling: the drive in the background,
the write through the server with the other persona's token once its first `state` is out:

```sh
bun exact.mjs agent web --json --storage live "clock data" state "clock +3000 real" "clock +5000" "clock data" state > .exact/live.jsonl &
until grep -q '"slots"' .exact/live.jsonl; do sleep 0.2; done
curl -s -X POST localhost:6410/cards -H 'authorization: Bearer bob' -H 'content-type: application/json' -d '{"title":"From Bob"}'
wait; tail -n 1 .exact/live.jsonl    # the last state holds Bob's card
```

`clock +5000` fires the app's poll (make it longer than the poll's interval) and `clock
data` lands its reply; `agent ios` is the same.
