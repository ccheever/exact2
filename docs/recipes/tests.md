# Recipe: tests

`app.test.contract` drives the app by `testId`, the same on the web and iOS (`bun
exact.mjs test web`, `test ios`; `verify` runs them too). Each
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
| `tap "id"` (`… contextmenu`) | Presses (long-presses) it; `tap "Milk"`: by text or label. A disabled control fails the step. |
| `type "id" "text"`, `type "id" key "Enter"` | Sets a value (a `select`'s, a date `"2026-10-10"`, a checkbox `"true"`); presses a key. |
| `expect text "id" == "…"` | Its text, else its value, else its descendants' text. |
| `expect tree has "id"` (`missing`) · `expect disabled "id"` (`enabled`) · `expect state groceries.0 .title == "Eggs"` | In the tree or not · a control's `disabled` · a state, derive or resource field. |
| `clock data` · `clock settle` · `clock +5000` · `reload` | Lands replies · ends transitions · moves the virtual clock · relaunches on the same storage, at the clock's time. |
| `back` | The platform's Back: cancels a dialog, else dismisses a sheet, else pops the route. |
| `fail fetch "<prefix>"`, `pass fetch …` | A lost connection there, then none. |
| `request POST "/items" {…} as grace expect 409` | Writes through the app's server as another person ([rest-backend](rest-backend.md)); a status not 2xx fails, or with `expect 409` any but 409. |
| `size 390x844`, `at "/item/7#moves"`, `epoch`, `time-zone`, `locale`, `seed`, `before data` | Launch lines: before a test's steps, or atop the file. |

Every other form (drags, `perf`, `on web`): [the grammar](../contract-grammar.md#authored-tests).

`expect` has no `<` or `contains`: compare a derive with `==`. `clock data` after an
input whose reply the next `expect` reads (iOS needs it); `clock settle` after a sheet, an
alert or a push. Test the refusal too; never wait in real time (pin `epoch`, `time-zone`); give
every screen its own ids; tap a tab before reading in it.

## Before you call it done

`verify` (`verify ios`) runs these tests, then `check`s the first screen and each test's end
screen, light and dark at two widths; on the web it reloads each end screen (a saved value that
comes back changed is a finding), then relaunches on a test's storage and checks what it stored
came back. Screenshots: `.exact/verify/` (`--shots <dir>` copies them). A clean `verify` is
done; a screen no test ends on is not checked, so end a test there. A finding that is the design
is listed, not counted, with `"verify": {"accept": [{"finding": "contrast", "node": "<testId>",
"why": "…"}]}` in app.json. Drive by hand to see why a step fails (`agent web --storage s1 …`).
