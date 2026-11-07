```prompt Make the keyboard work in my app
Make the text fields in the Exact app in this folder keep the keyboard up or put it away as the person expects. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/keyboard-and-focus.md as the source for each step. The exact2 checkout has the same page at docs/recipes/keyboard-and-focus.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. For each field, decide what Return does: add and keep typing, move to the next field, or put the keyboard away. If I did not say, propose it from what the app does and ask me before you write code.
3. In app.contract, Return calls the field's `submit` action; `enterkeyhint` labels the key, and `next` moves to the next field. To keep typing after Return, give the field an `id` and call `focus("<id>")` in its submit action. A sheet's first field takes `autofocus=true`.
4. While a field is edited (`focus=` and `blur=` actions that set a state), show a Done button that calls `blur("<id>")`. On a `list` or `scroll`, a drag that reaches the keyboard pulls it down; set `keyboardDismissMode="on-drag"` only to put it away as the drag starts. Put live results above the field.
5. Add a test in app.test.contract that types into a field, presses Return (`type "<id>" key "Enter"`), expects the result and the editing state, then taps Done and expects the editing state to be false.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me what Return does in each field, where Done shows, what the tests and checks reported, and the launch command for each target platform.
```
