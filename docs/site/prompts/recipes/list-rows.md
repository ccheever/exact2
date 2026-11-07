```prompt Add search, swipe and a menu to a list
Give a list in the Exact app in this folder a search field, swipe to delete, a filter and a long-press menu. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/list-rows.md as the source for each step. The exact2 checkout has the same page at docs/recipes/list-rows.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which list, and which of search, swipe to delete, a filter and a long-press menu it gets, propose them from the app and ask me before you write code.
3. Search: an `input type="search"` in the route's `header`, and a derive that filters the list. To query a server instead, read `bun exact.mjs contract vocab search`.
4. Swipe to delete: `delete=` on each row. It deletes without asking; a detail screen's Delete asks first (`bun exact.mjs contract vocab confirm`). For other swipe actions, read `bun exact.mjs contract vocab swipe`.
5. A filter: a `row role="radiogroup"` of `button role="radio"`. A long-press menu: one `popover="auto" role="menu"` for every row; each row's `contextmenu` notes which row, and each item acts on that row and hides the menu.
6. In app.test.contract, add a test for each one you added: type in the search and expect a row missing, tap `"<row testId>-delete"`, and tap a row with `contextmenu`, then a menu item.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. If the app runs on iOS, run `bun exact.mjs verify ios` too: it scrolls the list by real touches. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me what the list now does, what the tests and verify reported, and the launch command for each target platform.
```
