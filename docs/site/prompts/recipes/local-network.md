```prompt Find devices on my local network
Make the Exact app in this folder find a device on the local network (a light, a printer, a NAS) and reach it. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/local-network.md as the source for each step. The exact2 checkout has the same page at docs/recipes/local-network.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. If I did not name the device and its DNS-SD service type (for example `_wled._tcp`), ask me before you write code. Do not guess a service type.
3. Install the package: `bun <exact2>/scripts/exact.mjs add local-network .`.
4. In app.ts, declare the grants as the recipe shows (`device.local-network`, `net.dns-sd` for each service type, `net.fetch local-network`), and add the purpose text to `strings/en.json`. Resolve only the instance about to be used, each time, and reach it with `fetch`. Know a device by what it states, not by its instance name.
5. In app.contract, browse where the list is shown, and explain each state in the app's own words: `prompt` or `unknown` (the device asks once), `denied` (an Open Settings button), and not `available`. Always offer a typed address: the web, Linux and Windows do not browse.
6. In app.test.contract, add a test for the typed-address path and the text the app shows when browsing is not available. The web reads `available` false.
7. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me the service type, what the tests and verify reported, and how to see browsing by hand on macOS with a fake device, as the recipe's "Under the agent" command shows (`agent macos --permission local-network=prompt` with `prefer extension local-network.browseServices=…`). A fixture on 127.0.0.1 can stand in for the device.
```
