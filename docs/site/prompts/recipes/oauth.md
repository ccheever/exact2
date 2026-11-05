```prompt Add sign-in to my app
Add sign-in with OAuth to the Exact app in this folder. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/oauth.md as the source for each step. The exact2 checkout has the same page at docs/recipes/oauth.md.

1. If this folder is not an Exact app (it has no exact.mjs and app.contract), stop and tell me. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is.
2. Ask me first whether I have an account with a provider. If I do not, build against the fake provider, which runs on this machine, and tell me so. If I do, ask me for the provider (Google, Microsoft Entra ID, Auth0, Okta, OneLogin, GitHub, Salesforce, Amazon Cognito or ForgeRock), its client ID and issuer, the callback I registered, and the API the app calls with the token. Do not invent a client ID, and put no secret in the code.
3. Install the package: `EXACT_APP_DIR=$PWD bun <exact2>/scripts/exact.mjs add oauth`.
4. Read the provider's settings with `bun exact.mjs docs sign-in providers`. In app.ts, write the grants, `oauth({ ... })` and the sources as the recipe shows, with those settings. Write no token request, refresh or expiry check of your own.
5. In app.contract, add the account screen: a sign-in button, the person's data only while `session.signedIn`, and `problem` when it is not empty.
6. Test against the fake provider, as `bun exact.mjs docs sign-in providers` says under "Testing against a fake provider": use the profile for my provider, and add a test in app.test.contract for sign-in, a cancelled sign-in, sign-out, and staying signed in after a relaunch.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me what you changed, what verify reported, and what I must still do before a real sign-in works: the callback to register with the provider, and each line to change from the fake provider to the real one.
```
