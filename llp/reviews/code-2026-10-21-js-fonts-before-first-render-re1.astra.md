# Code review: the JS target's declared faces load before its first render (the clone's wasm-vs-JS widths), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `e4eb83c739`.
- **Method:** one brief (sha256 `2550e8081da5a0cd579a834e2142c1d6d6b4b40215d2860db0a9d8300947c500`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (the JS target's boot, the lead's 00:00Z task), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**RIGHT.**

No BLOCKING, SHOULD-FIX, or NIT findings within the requested scope.

[build.mjs:436](host/web-js/build.mjs:436) sets `clock.agent` synchronously before yielding to the font wait. Sounds and page-module callbacks therefore see the correct mode. `mount()` shares the predicate, and production’s `AGENT_ADMITTED` rewrite still disables `?agent`.

[fonts-boot.test.mjs:113](host/web-js/fonts-boot.test.mjs:113) requires non-null completion timestamps before checking widths; a legitimate timeout skips those assertions. The slow case holds responses until render, removes the upper timing bound, and checks `ready` settlement after mounting. Removing the deadline prevents rendering; removing or prematurely settling `ready` fails its assertions. Font preloads do not block navigation’s `load` event. [HTML specification](https://html.spec.whatwg.org/multipage/links.html#link-type-preload)

Read-only inspection and in-memory scheduling/assertion probes passed. Browser fixtures were not run because their builder writes files. HEAD and tree remain unchanged.
