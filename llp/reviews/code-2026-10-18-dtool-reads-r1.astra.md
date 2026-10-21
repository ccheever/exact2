# Code review: LLP 1122, the design tool's reads follow the files, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `a1c3ab7ee`.
- **Method:** one brief (sha256 `d573ba017a051fa8a9185461be17e56ec16c2948a2ca311103c8f8c3d355769a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium).
- **Disposition:** both taken, in `c06d9ee5b`:
  - a proposal follows the builds running (id, host, plan digest) and `hello.live` as well as the files; a contract test has a web-js build arrive under a Live draft of a token the stub's JS loop writes inline, and the chip turns Rebuilds (it fails without the key);
  - a read's describe and its last hello wait `READ_WAIT`, now `WRITE_WAIT` (130 s, past the server's 120 s).
  - Also, in `b9e3b1c3c` (after this round): a read asks four resolves at a time and stops once a newer read overtakes it, which bounds the queue a replacement describe waits behind.
- **Not taken:** a server-side build that stops before the design without any build event (Grok's case too) moves no key; no event says so, and the server judges each write.

I found one correctness regression to fix before landing, plus an incomplete timeout fix.

1. **[P2] Proposals still need invalidation for build and capability changes** — [app.contract:562](/tmp/rv-dt-reread/apps/design-tool/app.contract:562). A source-only rebuild can change a token’s usage to `font-weight` or an SVG attribute without changing any `.tamber` digest. The JS loop’s [inline check](/tmp/rv-dt-reread/host/web-js/dev.mjs:617) then changes the proposal from live to rebuild, but `filesAt` stays unchanged. The existing draft continues showing “Live” and offering a scrub that the server refuses. The reverse transition leaves scrubbing disabled unnecessarily. Changes to `hello.live` likewise do not recompute the chip, which reads the module’s `liveTable` variable. Give proposals a separate build/capability invalidation input while keeping expensive design reads file-driven.

2. **[P2] `describe` and the final `hello` still have the 15-second timeout** — [app.ts:223](/tmp/rv-dt-reread/apps/design-tool/app.ts:223), [app.ts:232](/tmp/rv-dt-reread/apps/design-tool/app.ts:232). Both share the serial server queue with resolves. A file change during a read can queue the replacement `describe` behind that read; another concurrent read can similarly delay the final `hello`. Using the actual client with a virtual queue of 73 × 207 ms resolves reproduced both failures: approximately 15.318 seconds for `describe` and 15.112 seconds for `hello`. Extend the timeout consistently across these read phases. The 120-second allowance covers the reported workload, but is per request and equals the server’s timeout without transport margin.

The other invalidation paths look sound: `changed` events—including same-digest round trips—first answers, gaps and epochs advance the file key. Explicit file selection remains an input. Writes, accepted imports and undo/redo also explicitly refresh design. Changeset/commit bookkeeping still follows `since`; branch, contrast and typed fields follow design reads; selection staleness still follows poll answers; highlights retain their fade trigger.

The test’s `pass: 1` and final restart are appropriate for the stated driver and shared-stub behavior. Its coverage is narrower than the change: no draft exercises `proposal`, and the default file makes `loadAppAxes` return without fetching.

Validated with in-memory probes using `app.ts`, the actual JS resource runtime and mocked I/O. No files changed; no builds or full contract tests ran.
