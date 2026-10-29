# Recipe: properties beside the tests

In `app.test.contract`, state what should hold across the app's interactions:

```contract-test
always "the phone is on one of its five screens"
  expect state screen in "lock" "home" "settings" "display" "messages"

always "the counter is a whole number of at least one"
  expect state counter whole
  expect state counter >= 1

always "a probe's result is absent or an answer"
  expect state tsResult in none some

always "once the photo has loaded it stays loaded"
  expect state ready stays true

after "increment" "increment adds one"
  expect state counter == old + 1

after "rotate" "rotating touches nothing but the turns"
  only writes turns

after "done" "done writes nothing and focuses the button"
  writes none
  command focus "done"

never refuses "reset"
```

Choose the blocks your app means. Names read root states or derives; no fields,
indices or expressions. A derive must depend only on state and pure functions:
resources, mutation status, the clock and geometry are refused with the dependency
chain. `stays true` means that a true value cannot become false at the next observed
step. `after` checks an eligible commit and its landed continuations; extra commits
can make a line counted only. The [grammar](../contract-grammar.md#authored-tests)
defines each form.

Run `bun exact.mjs contract verify app.contract`. It delivers test prefixes, explores
controls and searches 20 walks of up to 40 chosen events, plus automatic clock
advances. Set `--walks`, `--steps` and
`--walk-seed` to reproduce another search; `--confirm` sets how many walks Lean
confirms, alongside delivered tests, explore and shrunk findings. Without `lake`,
or with `--runner-only`, the runner still searches. `--js` also runs the scripts
on the web JS target and checks `always` there.

The receipt gives the command, app/test/embedding digests, scripts
delivered or stopped, steps, stores, action coverage, property results, refusals,
timings and which evaluators ran. A property can be held, false, counted only or
not exercised. Suggestions describe observations, not proofs; they do not edit tests.
Keep the receipt unchanged in your diary;
its digests tell a reader when the sources have moved.

“In the semantics' world” means seeded source answers arrive synchronously and
never fail. The events are taps, typing, landing continuations and advancing the
clock. Reloads, late answers and host inputs are outside that world. A property
about a mutation result is therefore about the seeded answer, and a successful
search establishes only what those scripts observed.

`bun exact.mjs test web` (or `test ios`) checks `always` at boot, after inputs and
reloads, and after every clock step,
including automatic test-clock advances.
`bun exact.mjs verify` runs the device checks and then `contract verify`, and either
failure fails the command. Device and headless JS runs report how many `after`,
frame, command and `never refuses` lines they leave unchecked: neither exposes
the commit trace those lines need.
