```prompt Add a capability that asks permission
Add a capability that needs the person's permission (location here) to the Exact app in this folder. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/permissions.md as the source for each step. The exact2 checkout has the same page at docs/recipes/permissions.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. If I did not say what the app does with the location, propose it and ask me, and ask whether it needs a precise or an approximate location.
3. Install the module: `bun <exact2>/scripts/exact.mjs add geolocation .`.
4. In app.ts, declare one grant line (`device.geolocation-approximate` or `device.geolocation`, not both) with its purpose key, and add the purpose sentence to `strings/en.json`. Write no platform manifest.
5. In app.contract, read `geolocationStatus()` as a fact, ask only from a person's press, and while the state is `denied` say so and offer Open Settings. Everything that does not need the location keeps working.
6. Add tests in app.test.contract for each state, with the agent's answers the recipe lists: allow (`tap @t allow`), deny, a denied launch with its Settings offer, and a position (`type @t <latitude>,<longitude>`).
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me the grant line and purpose you wrote, what the app does in each state, and what the tests and verify reported. `apps/weatherlight` in the exact2 checkout is the whole example.
```
