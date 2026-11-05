```prompt Add a navigation stack
Give the Exact app in this folder a navigation stack: a list that pushes a detail, a sheet to add, and an alert before a delete. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/navigation-stack.md as the source for each step. The exact2 checkout has the same page at docs/recipes/navigation-stack.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not name the screens, propose them from the app's data (the list, the detail, the sheet) and ask me before you write code.
3. In app.contract, make each screen a route under `routes nav`, never an `if` over state: a route gets Back, links and split. Draw them in a `for` over `stack(nav)`, each with its own `navigationKey`.
4. Give each route a `header` with one heading: `aria-level=1` for a large title, 2 for an inline one. A list row that only navigates is a `link href=path(…)`, pushed by a `follow` action.
5. Make the sheet a route with `navigationPresentation="modal"`, and its Cancel `id="back"`, the root's `navigationBack`. Ask before a delete with a `dialog role="alertdialog"`, its `aria-label` the question.
6. Keep each `testId` unique: routes stay mounted. In app.test.contract, add a test that adds an item, opens it, and deletes it after the alert. Use `clock settle` after a push, a sheet or an alert, and `clock data` before you read a save.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me the routes and what each holds, what the tests and verify reported, and the launch command for each target platform.
```
