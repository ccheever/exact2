# Code review: the clone's live reads on a local server, round 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `73487b9eed`.
- **Method:** one brief (sha256 `5f7efb681dc1e02c972b16da13d3791ba60dda86f6f0fe74894fb7b9ab4568b5`), blind to the other reviewer; 2 rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** The brief also covered the clone's split (live-split 32377a3).

No BLOCKER found.

- **SHOULD — [scripts/live-reads-fixture.mjs:205](scripts/live-reads-fixture.mjs:205): the delay does not guarantee an in-flight request at the clock call.** A slow driver can consume the 300 ms before reaching `clock data`, allowing a broken wait to pass. The scratch breaks establish sensitivity under the tested timing. For a reliable guarantee, control response release and check that the clock cannot complete while the relevant response remains withheld.

- **SHOULD — [clone/app.test.contract:36](split/app.test.contract:36): live reads remain in the default suite.** “The butterfly scrolls to the top…” still loads and reloads Discover. “Search history on the device…” at line 47 also triggers actor typeahead when settlement advances its debounce. More broadly, all eight retained tests launch the root Discover resource; `before data` suppresses waiting, not fetching. Block or stub these reads if the default suite must be network-independent. Most retained assertions tolerate failed reads, but the split does not eliminate network waits.

- **SHOULD — [host/web/live-reads.test.mjs:14](host/web/live-reads.test.mjs:14), [host/web-js/live-reads.test.mjs:14](host/web-js/live-reads.test.mjs:14): setup can leak servers.** Both acquire resources before entering `try`. A directory creation/write failure leaks the JS endpoint server; wasm’s `mkdtempSync` failure leaks both servers, including port 47712. Put acquisition under cleanup coverage. Normal completion is cleaned up; concurrent wasm runs still require distinct ports and matching builds.

- **NIT — [scripts/live-reads-fixture.mjs:245](scripts/live-reads-fixture.mjs:245): describe these as synthetic runtime cases.** The clone’s [Discover poll](split/data/latest.ts:42) returns locally, and [signed-out Top/Latest](split/data/explore.ts:259) makes no request. Neither exercises the fixture’s `.getFeed.poll` or 403 mapping. Two existing stubbed tests confirm this. The fixture’s opening comment also states network causality more strongly than the supplied evidence supports.

Otherwise, the negative assertions require landed failures; the 61-second jump crosses one poll interval; the drag has sufficient content; reload keeps storage while replacing the document; and `n=reread` makes duplicate requests harmless. Config failure correctly hides switches. The polling assertions exercise the jump’s wait—the subsequent `clock data` is not independently responsible.

Both diffs pass whitespace checks. All 11 clone blocks are preserved unchanged across the split; changed files satisfy the line cap. Two existing stubbed tests passed. New browser suites were not rerun under read-only constraints.
