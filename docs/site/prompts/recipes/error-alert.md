```prompt Tell the person when something fails
Make the Exact app in this folder tell the person in an alert when a save or another action fails. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/error-alert.md as the source for each step. The exact2 checkout has the same page at docs/recipes/error-alert.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which failure, list the mutations in app.contract and ask me which ones the person must acknowledge.
3. Make the source in app.ts answer `{ ok, error }` for each refusal, with a short message that says why.
4. In app.contract, add a `problem` state and give the mutation `then` an action that sets `problem` and calls `showModal` when the answer is a refusal. Draw a `dialog role="alertdialog"` with a short `aria-label` as its title, the problem as its text, and one OK button with `commandfor` and `command="close"` and no `press`.
5. `then` does not run when a request brings no answer: show `failed(<mutation>)` on the screen for that case. Keep long text out of the alert; it belongs in a sheet.
6. Add a test in app.test.contract that causes the refusal, runs `clock data` and `clock settle`, expects the message, taps OK, and expects the person's input to stay.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me which failures show an alert and their text, what the tests and verify reported, and the launch command for each target platform.
```
