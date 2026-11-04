```prompt Add sign-in to my app
Add sign-in with OAuth to the Exact app in this folder. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/oauth.md as the source for each step. The exact2 checkout has the same page at docs/recipes/oauth.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. Ask me for the provider (Google, Microsoft Entra ID, Auth0, Okta, OneLogin, GitHub, Salesforce, Amazon Cognito or ForgeRock), its client ID and issuer, the callback I registered, and the API the app calls with the token. Do not invent a client ID, and put no secret in the code. If I have no provider yet, build against the fake provider and tell me so.
3. Install the package: `EXACT_APP_DIR=$PWD bun <exact2>/scripts/exact.mjs add oauth`.
4. Read the provider's settings with `bun exact.mjs docs sign-in providers`. In app.ts, write the grants, `oauth({ ... })` and the sources as the recipe shows, with those settings. Write no token request, refresh or expiry check of your own.
5. In app.contract, add the account screen: a sign-in button, the person's data only while `session.signedIn`, and `problem` when it is not empty.
6. Test against the fake provider, as `bun exact.mjs docs sign-in providers` says under "Testing against a fake provider": use the profile for my provider, and add a test in app.test.contract for sign-in, a cancelled sign-in, sign-out, and staying signed in after a relaunch.
7. Run `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
9. Tell me what you changed, what verify reported, and what I must still do before a real sign-in works: the callback to register with the provider, and each line to change from the fake provider to the real one.
```
