```prompt Make my columns wrap on a phone
Make the columns in the Exact app in this folder sit side by side on a wide window and stack on a phone. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/responsive-columns.md as the source for each step. The exact2 checkout has the same page at docs/recipes/responsive-columns.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not name the screen, find the one with columns side by side and ask me before you write code.
3. In app.contract, put the columns in a `row flex-wrap="wrap"` and give each column `flex="1 1 280px" min-width=0`, or a basis that fits its content. Never squeeze columns to fit; columns that scroll sideways are a `scroll overflow-x="scroll"` of fixed-width columns. A list beside its detail is `bun exact.mjs pattern list-detail`.
4. A title and a button on one line: wrap their row too, and give the title `flex="1 1 12rem"`, so the button moves under the title at the largest text size.
5. Add tests in app.test.contract: one with `size 1000x700` and one with `size 390x844`, each expecting every column (`expect tree has "<column id>"`).
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me the screen, the column widths, what the tests and checks reported, and the launch command for each target platform.
```
