```prompt Add a transfer the system finishes
Make the Exact app in this folder hand an upload or a download to the system, which finishes it after the app is suspended or terminated. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/background-transfers.md as the source for each step. The exact2 checkout has the same page at docs/recipes/background-transfers.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. If I did not say which transfer to hand over, propose it and ask me. Background transfers run on iOS and macOS; on the web `available` is false and the app uses `fetch`. If the app runs on neither iOS nor macOS, tell me and stop.
3. Install the package: `bun <exact2>/scripts/exact.mjs add background-transfer .`.
4. In app.ts, start the transfer with `backgroundFetch` from data code only, and grant each `url`, `bodyFrom` and `saveTo` (`net.fetch`, `fs.read`, `fs.write`). Put no credential in app.contract.
5. On the root, handle `backgroundfetchsuccess`, `backgroundfetchfail` and `backgroundfetchabort`. Keep what the transfer carried, then call `retireBackgroundFetch`; until then the event comes again at later launches.
6. Add tests in app.test.contract for a transfer that succeeds and one that fails, and on the web for the `fetch` path. `packages/background-transfer/test` in the exact2 checkout is a whole example.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes; then `bun exact.mjs verify ios` if the app runs on iOS. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me which transfer the system now finishes, the grants you added, and what the tests and verify reported.
```
