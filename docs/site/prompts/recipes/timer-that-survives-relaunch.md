```prompt Keep a timer running across a relaunch
Make a timer in the Exact app in this folder keep running, and keep its time, when the app closes and opens again. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/timer-that-survives-relaunch.md as the source for each step. The exact2 checkout has the same page at docs/recipes/timer-that-survives-relaunch.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If the app has no timer, or I did not say which one, ask me before you write code.
3. In app.contract, keep the start mark as a date, `time.epochAtZero + performanceNow()` from an `exactTime()` resource, in a root `state startedAt = 0 persist`. Never store `performanceNow()`: it starts at 0 on every launch. Keep the time before a pause as a duration in `state banked = 0 persist`. Write no data module.
4. Derive the elapsed time from `banked`, `startedAt` and `now`, and run a `task` with `every(250, step)` only while the timer runs. Read `performanceNow()` in the start and stop action itself, not the `now` the task wrote.
5. Show the readout with `font-variant-numeric="tabular-nums"` and its size in `rem`.
6. Add the recipe's two tests in app.test.contract: with `epoch` set, start, `clock +5000`, `reload`, and expect the same time and start mark; then stop, `reload`, expect the banked time, and reset.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me which timer now survives a relaunch, what the tests and verify reported, and the command that opens the app: `bun exact.mjs web`.
```
