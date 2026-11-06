```prompt Add a grouped form
Add an entry or settings form to the Exact app in this folder: a grouped list of fields, with Save disabled until the form is valid. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/grouped-form.md as the source for each step. The exact2 checkout has the same page at docs/recipes/grouped-form.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not name the fields and what makes the form valid, propose them from the app's data and ask me before you write code.
3. In app.contract, make the form a `list listStyle="inset-grouped"` of `section`s, a `state` per field. Use the platform's controls and leave their look unsaid: `input` for text, `input type="number" min max step` for a quantity, `input type="checkbox" switch` for a switch, `input type="date"` for a date, and `select` for a menu. A number field's value is text: read it with `parseNumber`.
4. Put Save in the header, disabled while the form is not valid, and say why in the section's `footer`. Give the last field `submit=save`, so Enter saves.
5. In app.test.contract, add a test that Save is disabled and the reason shows, then that a valid entry enables Save, and that Enter saves.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me the fields and the rule for a valid form, what the tests and verify reported, and the launch command for each target platform.
```
