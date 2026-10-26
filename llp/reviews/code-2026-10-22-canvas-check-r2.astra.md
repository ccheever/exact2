# Code review: tamber tool --via's canvas check (a page that won't let the tool frame it), round 2 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 2 of 2; HEAD `9936d5baa9`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; two SHOULD-FIXes (the advice from stripped names, ports as numbers) and a NIT (schemeless naming), taken in 12d92fdfa7.

No BLOCKER findings. Three remaining issues:

1. **SHOULD-FIX — [scripts/tamber-frame.mjs:80](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:80)** (also [canvas-check.ts:15](/tmp/rv-dt-framecheck/apps/design-tool/canvas-check.ts:15)): Port advice uses origins after paths are stripped. `frame-ancestors http://127.0.0.1:7740/private` therefore recommends `EXACT_TOOL_PORT=7740`, although no port fixes that restriction. Derive the advice before discarding path information.

2. **SHOULD-FIX — [scripts/tamber-frame.mjs:48](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:48)**: Explicit ports are compared as strings. `http://127.0.0.1:07740` incorrectly refuses a tool on port 7740, producing an empty-canvas warning when framing is permitted. Normalize numeric ports before comparison, as [CSP’s port-matching algorithm](https://www.w3.org/TR/CSP3/#match-port) requires.

3. **NIT — [scripts/tamber-frame.mjs:61](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:61)**: Round-one answer 4 remains incomplete for schemeless sources: `127.0.0.1:9999/private` becomes `127.0.0.1:9999`, not `http://127.0.0.1:9999`. Supply the page’s scheme; its absence also changes which advice appears.

The other listed fixes look consistent. No files changed; no builds or tests run.
