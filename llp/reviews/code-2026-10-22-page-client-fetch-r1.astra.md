# Code review: the JS dev loop's page client keeps the page's own fetch, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `89ea69eb36`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one SHOULD-FIX (the test clicks after a fixed wait) and a NIT (a refused register stays silent), both taken.

No BLOCKER found.

1. **SHOULD-FIX — [host/web-js/dev-page-streams.test.mjs:116](/tmp/rv-dt-devfetch/host/web-js/dev-page-streams.test.mjs:116):** The fixed one-second sleep leaves a race. `hello.builds[].inspector` records server ownership before the browser receives the command and activates inspect mode. A delayed import/SSE delivery lets the click reach the app, then the test times out. Wait for `#exact-inspect` to be visible before clicking. The left-edge click avoids the centered banner at the default viewport.

2. **NIT — [host/web-js/dev.mjs:730](/tmp/rv-dt-devfetch/host/web-js/dev.mjs:730):** HTTP registration failures remain silent: a 400/403/500 resolves with `ok === false`, bypassing the new `.catch()`. Warn on non-OK responses too, including the status.

The capture precedes app execution on the normal, gallery, checkpoint-reload, and copy-iframe paths. The client’s only direct module import is `inspect.js`; both fetch calls there are covered. Preinstalled fetch stubs remain captured; later replacements are bypassed for dev traffic as intended.

Read-only review; no builds or tests run.
