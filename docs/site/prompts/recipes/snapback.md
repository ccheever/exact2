```prompt Connect my app to a Snapback backend
Give the Exact app in this folder shared server data from a Snapback backend. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/snapback.md as the source for each step. The exact2 checkout has the same page at docs/recipes/snapback.md.

1. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is. Ask me where the Snapback backend is and which port its origin uses (the default is 4400), and whether the schema is server-only. Do not guess them.
2. If this folder is an Exact app (it has exact.mjs and app.contract), wire it: `bun exact.mjs snapback`. If it is not, ask me before you make one; then, from the backend's directory, run `bun <exact2>/scripts/exact.mjs new <name> --snapback`, with `--port <n>` if the port is not 4400. For a server-only schema, add `--online`.
3. Select another local backend with `bun exact.mjs snapback --port <n>`, `EXACT_BACKEND_ORIGIN` or `bun exact.mjs test web --backend <url>`, not by an edit to app.ts or to the tests. A computed origin or grants declaration needs an explicit edit: tell me before you make it.
4. Write the screens with the generated operations that `snapback` mounts. For password sign-in, guests or personas, use docs/recipes/snapback-auth.md and tell me first.
5. Start `snapback4 dev` on the origin's port, as `bun exact.mjs snapback` names it: `test web` never starts a backend. Write tests in app.test.contract as docs/recipes/snapback-tests.md says.
6. If a command refuses a stale device, or after Snapback4 was upgraded, run `bun exact.mjs snapback --rebuild`.
7. Follow the platform checks in docs/start-here.md (`bun exact.mjs docs platform checks`) for the targets in the app's AGENTS.md and app.json until they pass. Read the reports, including counted findings; a zero exit code alone does not mean clean. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 8) and report it; do not keep trying.
8. When a doc was wrong or missing, or a step needed a workaround, read `bun exact.mjs feedback status`. Under `never`, keep no note. Otherwise record it locally: `EXACT_FEEDBACK=0 bun exact.mjs feedback note "<what happened>"` (use your shell's environment syntax). Do not change the saved policy or send notes, even under `always`; tell me how many you kept.
9. Tell me what you wired and the origin it uses, the command that starts the backend, what the tests and verify reported, and the launch command for each target platform.
```
