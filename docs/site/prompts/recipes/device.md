```prompt Use the battery, the network and Low Power Mode
Make the Exact app in this folder read the battery, the network and Low Power Mode, and use less when the person or the device asks. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/device.md as the source for each step. The exact2 checkout has the same page at docs/recipes/device.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. If I did not say what the app changes for each fact, propose it (for example, stop an animation in Low Power Mode, or defer an upload under Save Data) and ask me.
3. Install the package: `bun <exact2>/scripts/exact.mjs add device .`. None of the three facts asks permission.
4. In app.contract, read `getBattery()`, `connection()` and `powerState()` as resources, each with a placeholder no answer can be (`level=-1`, `type="unknown"`), so a failed read draws what the app draws without the fact. `powerState()` fails on the web.
5. What the person presses wins over the device: keep the person's choice in a state, and let the device decide only while there is none. In data code, read the same sources as Promises.
6. Add tests in app.test.contract with `prefer extension device.powerState={"lowPowerMode":true}` and `=clear`: the app uses less in Low Power Mode, returns after, and keeps the person's choice.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me which facts the app reads, what it changes for each, and what the tests and verify reported.
```
