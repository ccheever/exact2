```prompt Let a person add and remove rows
Let a person add and remove rows in a list of the Exact app in this folder while they edit it, such as a poll's options or a recipe's steps. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/editable-rows.md as the source for each step. The exact2 checkout has the same page at docs/recipes/editable-rows.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which list, propose one from the app's data, with its fewest and most rows, and ask me before you write code.
3. In app.contract, draw the rows in a `section` of an inset-grouped `list`, with an `add` action, a `remove(i)` action and an `edit(i, v)` action. Give each row a red remove button (`destructive=true`, `symbol:minus-circle`, an `aria-label` that names the row) and its swipe (`delete=remove(i)`). Show the remove button and allow the swipe only while the list has more than its fewest rows.
4. Make Add the section's last row, and show it only while the list has fewer than its most rows.
5. A removed row asks nothing: the form's Save or Cancel is the commitment. If the form saves something that cannot be undone once saved, it asks first (`bun exact.mjs contract vocab confirm`).
6. Add a test in app.test.contract that adds a row, types in it, removes another, expects the values that stay, and expects no remove button at the fewest rows.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me which list can change and its limits, what the tests and verify reported, and the launch command for each target platform.
```
