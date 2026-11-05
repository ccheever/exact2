```prompt Add guests, personas or password sign-in
Add identity to the Exact app in this folder, on its Snapback backend: guests, development personas or password accounts. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/snapback-auth.md as the source for each step. The exact2 checkout has the same page at docs/recipes/snapback-auth.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), or it is not wired to a Snapback backend, stop and tell me: wire it first with docs/recipes/snapback.md.
2. Ask me which identities the app allows: guests and password accounts (`use identity`), password accounts only (`use identity password`), and whether development uses personas (`use personas alice, bob`). Do not choose for me.
3. Build from the complete mixed identity app in the exact2 checkout's snapback4/README.md ("A complete mixed identity app"): the schema, Contract, TypeScript and tests.
4. For guests, call `guest()` before a read. Keep the session with `auth: {store, key, now}` and grant `secret.keep` for that key. Use a distinct secret key for each backend origin.
5. For personas, start with `state persona = ""` (signed out) and an explicit persona choice. Await logout before a switch of accounts. Hide retained rows until the current principal's resource answers.
6. `serve` refuses persona headers: tell me the persona controls must go before production.
7. In app.test.contract, add the recipe's test, with `fresh` and `epoch now` at the top of the file. A test that expected a persona after `reload` now expects signed out, then chooses the persona again.
8. Start `snapback4 dev` on the backend's port, as `bun exact.mjs snapback` names it. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 9) and report it; do not keep trying.
9. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
10. Tell me which identities the app allows, what you must remove for production, what test web and verify reported, and the command that opens the app: `bun exact.mjs web`.
```
