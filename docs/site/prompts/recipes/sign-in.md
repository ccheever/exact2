```prompt Add Sign in with Apple and Google
Add Sign in with Apple and Google to the Exact app in this folder. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/sign-in.md as the source for each step. The exact2 checkout has the same page at docs/recipes/sign-in.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. Ask me which providers to offer, and for each one its client IDs (Apple's Services ID; Google's web client and iOS client) and my server's URL. If I have none yet, build against the test provider the recipe names (`packages/sign-in/test/idp.mjs`) and tell me so. Do not invent a client ID, and put no secret in the code.
3. Install the package: `bun <exact2>/scripts/exact.mjs add sign-in .`. Copy from `apps/auth-fixture` in the exact2 checkout (`app.contract`, `app.ts`, `sign-in.mjs`).
4. In app.ts, write a grant line for each client and `net.fetch` for the server. Fetch a nonce when the screen shows, and call `get` at the press with nothing awaited before it. Post the credential without its `handle`, and fetch a fresh nonce after every request.
5. In app.contract, show the provider's own button (`exact-sign-in-button`) only while the provider is active; never draw the button yourself. Show nothing for `NetworkError`, `NotAllowedError` or `AbortError`. Sign out with `preventSilentAccess()`.
6. On the server, check the signature, `iss`, `aud`, `exp` and the nonce, once, as step 4 of the recipe says, and keep Apple's `user.name` the first time. For the web, add the return route of step 5.
7. Add tests in app.test.contract for a sign-in, a cancelled sign-in (`tap @t cancel`) and a sign-out. Under the agent, each request holds a `credential` ticket.
8. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 9) and report it; do not keep trying.
9. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
10. Tell me what you changed, what the tests and verify reported, and what I must still do before a real sign-in works: each client to register, and each line to change from the test provider to the real one.
```
