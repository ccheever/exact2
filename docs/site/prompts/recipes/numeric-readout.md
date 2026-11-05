```prompt Add a big number readout
Add a big number readout to the Exact app in this folder: a value such as a timer or a running total, shown large and updated in place. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/numeric-readout.md as the source for each step. The exact2 checkout has the same page at docs/recipes/numeric-readout.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which number to show and on which screen, ask me before you write code.
3. In app.contract, show the number in a `text` with its size in `rem` (`font-size="4.5rem"`), `font-variant-numeric="tabular-nums"`, `-exact-fit="shrink"` and `white-space="nowrap"`, and an `aria-label` that says what the number is.
4. Design only the number's size. Leave its colour unsaid or use `color="AccentColor"`, never a hex. Give its label `color="-exact-secondary-label"`. A title stays a heading, never a big `font-size`. If content beside the number must stay still when the digit count changes (`99` to `100`), reserve space for the widest value.
5. Add a test in app.test.contract that expects the readout's text, runs the action that changes it, and expects the new text.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. verify also checks each screen at the largest text size: the readout must stay on one line there. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me the readout, what it shows, what the tests and checks reported, and the launch command for each target platform.
```
