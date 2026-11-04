```prompt Add an edit sheet
Let a person edit an item in the Exact app in this folder, in a sheet. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/edit-sheet.md as the source for each step. The exact2 checkout has the same page at docs/recipes/edit-sheet.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which item and which fields to edit, propose them from the app's data and ask me before you write code.
3. In app.contract, add an edit route with the item's id (`"/edit/:id"`), under the route or tab that holds the list, drawn with `navigationPresentation="modal"`. Open it with a `link href=path("edit", id)`: from the row, or, if the row already opens another screen, from an Edit link on that screen.
4. Make the form a child component that takes the item, a save action and a cancel action as props, with a `state` per field that starts from the item. Draw it inside a `for` over the matching item with `key` set to the item's id. Do not fill a draft from an action: a link, a deep link and Back never run it.
5. Give the sheet Cancel (`id="back"`, the root's `navigationBack`) and Save, which is disabled while the form is not valid. Save writes the item and goes back. If the items live in app.ts, Save sends a mutation that updates the item there, and the list's resource refreshes.
6. Add a test in app.test.contract that opens the first item's sheet, expects its values, changes one, saves, and expects the new value; then opens a second item's sheet and expects that item's values, not the first's.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me what can be edited and from where, what the tests and verify reported, and the launch command for each target platform.
```
