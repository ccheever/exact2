```prompt Give a component a timer
Move a timer that a component needs into the root of the Exact app in this folder, so the component can start and stop it. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/timer-in-a-component.md as the source for each step. The exact2 checkout has the same page at docs/recipes/timer-in-a-component.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me.
2. If I did not name the component that needs the timer, find it from what the app does and ask me before you write code.
3. Only the root component may declare a `task`. Move the `task`, the state that gates it (its `when`), and the actions that set that state and step the timer to the root.
4. Give the component that state and those actions as props (`name: action`), and bind each argument where the component is used (`down=hold(true)`). The component keeps its view and never writes the root's state. Two components with timers take two tasks, or one task on either flag (`when leftHeld or rightHeld`).
5. Keep the existing tests passing without changing what they check. Add a test in app.test.contract that starts the timer (`tap "<id>" drag 0 0 hold <ms>` for a held control, or a `tap`), expects state while it runs, and after it stops runs `clock +<ms>` and expects the state unchanged.
6. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 7) and report it; do not keep trying.
7. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
8. Tell me which task moved to the root, the props the component now takes, what the tests and checks reported, and the launch command for each target platform.
```
