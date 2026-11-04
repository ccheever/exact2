```prompt Add properties to my tests
Add properties to the Exact app in this folder: rules in app.test.contract that must hold across every interaction. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/properties.md as the source for each step. The exact2 checkout has the same page at docs/recipes/properties.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. Read app.contract and list the rules the app means: the values a state may take, what an action writes and what it leaves alone, what never changes once true, and which actions must never be refused. Show me the list and ask before you write them.
3. A property names root states or derives only: no fields, indices or expressions. A derive it names must depend only on state and pure functions, never on a resource, a mutation's status, the clock or geometry. If a rule needs data from app.ts, it cannot be a property: tell me, and leave it to a test.
4. Write the rules in app.test.contract with the recipe's blocks: `always`, `after "<action>"`, `never refuses`, and `only writes` or `writes none`. Change no existing test and nothing in app.contract.
5. Run `bun exact.mjs contract verify app.contract`. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json. Read the reports, including counted findings; a zero exit code alone does not mean clean. A property that is false is a bug in the app or a wrong rule: show me the steps that broke it and ask which one before you change anything. Do not weaken a property to make it pass.
6. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me each property and its result (holds, false, counted only or unexercised), the rules you could not write as properties and why, and what verify reported. Keep the `contract verify` receipt unchanged in the app's diary.
```
