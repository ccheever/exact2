```prompt Keep a setting across launches
Make the Exact app in this folder remember a setting across launches. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/persisted-setting.md as the source for each step. The exact2 checkout has the same page at docs/recipes/persisted-setting.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not say which setting to keep, list the choices the app already has and ask me which ones. A setting is a choice the person makes (a filter, a sort order, units, a toggle), not text they are typing or a draft.
3. For each setting, add `persist` to its root `state` in app.contract. If the setting is a child component's state, move it to the root and pass it down as a prop. Persist only a number, string or bool, or an option or list of one; never a record, a list of records or a derive. Write no data module, grant or storage code.
4. If a setting is a choice of one value, make it a `radiogroup` of radios, each `aria-checked` when it is the value.
5. In app.test.contract, add a test that changes each setting, runs `reload`, and expects the new value, and a test that a fresh install starts from the declared value.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me which settings the app now keeps, what the tests and verify reported, and how to change a setting and reopen the app on each target platform using the same named store. Add `--keep-storage` when reopening a web store. Report any platform you could not check.
```
