```prompt Show loading, error and retry for a fetch
Make a screen of the Exact app in this folder show loading, an error with Try Again, and the empty state for data it fetches. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/fetch-loading-error.md as the source for each step. The exact2 checkout has the same page at docs/recipes/fetch-loading-error.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which data, list the resources in app.contract that fetch from the network and ask me which one. If the app fetches nothing yet, ask me for the URL; do not invent one.
3. In app.ts, add the URL to `grants` (`net.fetch <origin>`), set `exactTimeout` on the fetch, throw when the response is not ok, and check the response's shape before you return it.
4. In app.contract, draw a `progress` while `pending`, a message and a Try Again button while `failed`, and an empty-state line when the answer is valid and empty. Give the scroll `refresh` and `refreshing`. A failed refresh keeps the old rows beside Try Again.
5. If the resource takes an account, a server or a workspace, the answer names its owner, and the screen shows rows only when the owner matches the current one. An owner is never empty.
6. Add a test in app.test.contract like the recipe's: `fail fetch` on the origin, refresh, expect the loading state, the kept rows and the error; then `pass fetch`, tap Try Again, and expect no error. `fail fetch` cannot give an HTTP status or body; if a test needs one, tell me.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me which screen changed and what it shows in each state, what the tests and verify reported, and the command that opens the app: `bun exact.mjs web`.
```
