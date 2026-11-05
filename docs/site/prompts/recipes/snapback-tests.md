```prompt Test my app against its Snapback backend
Add tests to the Exact app in this folder that run against its Snapback backend. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/snapback-tests.md as the source for each step. The exact2 checkout has the same page at docs/recipes/snapback-tests.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), or app.ts grants no loopback `net.fetch` origin for a Snapback backend, stop and tell me: wire it first with docs/recipes/snapback.md.
2. Start `snapback4 dev` on that origin's port, as `bun exact.mjs snapback` names it, before the checks run: the runner refuses a Snapback run when none listens. `EXACT_PORT` moves the page's port, never the backend's.
3. Backend rows survive tests and runs. For tests that need an empty backend, put `fresh` at the top of app.test.contract, or `fresh "seed.json"` to seed it. This needs Snapback4 0.4.33. A password-only app's `dev` opens reset only with `--unsafe-dev-doors`: ask me before you use it.
4. If a table allows no `insert` (a role roster), seed its rows in seed.json for each test. Each seed row needs a stable 26-character Crockford base32 `id`. Show me the sample roster rows (persona, role and fields) and ask before you use them.
5. Server ids are unique, not ordered: reach rows by text or by position (``testId=`task-${i}` ``), never by id order. Use `epoch now` unless a test arranges both clocks.
6. For each write the server refuses, add a test: `clock data`, then `expect refusal "<mutation slot>"`, then expect the code or message the app shows.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me each test and what it checks, the command that starts the backend, the seed you used, and what the tests and checks reported.
```
