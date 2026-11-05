```prompt Add tests to my app
Add tests to the Exact app in this folder, in app.test.contract, so verify checks each screen. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/tests.md as the source for each step. The exact2 checkout has the same page at docs/recipes/tests.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. Read app.contract and list what a person does on each screen, and each refusal the app makes (an empty or repeated entry, a lost connection). Show me the list and ask before you write the tests.
3. Give every screen and control the tests use its own `testId`. Change nothing else in app.contract.
4. Write one test for each item with the recipe's steps: `tap`, `type`, `expect text`, `expect tree has`, `expect state`, `back`, `reload` and `fail fetch`. Put launch lines (`size`, `at`, `epoch`, `time-zone`) first. `expect` has no `<` or `contains`: compare a derive with `==`.
5. Add `clock data` after an input whose reply the next `expect` reads, and `clock settle` after a sheet, an alert or a push. Never wait in real time: pin `epoch` and `time-zone`. Tap a tab before you read inside it.
6. End a test on each screen: verify checks only the screens a test ends on.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. A failing test is a bug in the app or a wrong test: to see why a step fails, drive by hand with `bun exact.mjs agent web --storage s1 "<op>"`, then show me the step and ask which one before you change anything. Do not add a `verify.accept` entry to app.json without asking me. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me each test and what it checks, any screen no test ends on, and what the tests and checks reported.
```
