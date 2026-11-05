```prompt Run work while the app is unseen
Make the Exact app in this folder run work while it is unseen: a one-time sync, a periodic sync, or both. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/background-work.md as the source for each step. The exact2 checkout has the same page at docs/recipes/background-work.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. If I did not say which work to run, propose it and ask me. A `sync` runs once at the next chance, which on an iPhone can be hours later. A `periodicsync` runs no sooner than its `minInterval` (at least 15 minutes), and never on the web.
3. Install the package: `bun <exact2>/scripts/exact.mjs add background .`. In app.ts, declare the grant lines bare, with no purpose (`device.background-sync`, `device.periodic-background-sync`).
4. Register the tag from data code or an action. On the root, handle `sync` and `periodicsync` by sending to data code. The work must settle in 30 s, and it may not open system UI, ask the person or play sound. To retry, register the tag again from the handler.
5. If the app has a switch for periodic work, show it only while `background.periodic` is true.
6. Add tests in app.test.contract that run each registration with `tap @<ref>` from `state.backgroundSync` and check what the work did, and that a tap while offline (`prefer online false`) is refused. Harmony's `packages/background/test/harmony.mjs` in the exact2 checkout is a whole example.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me each tag, when it runs on each platform, and what the tests and verify reported.
```
