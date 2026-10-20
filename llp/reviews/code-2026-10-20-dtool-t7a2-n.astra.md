# Code review: LLP 1122 T7a-2, the narrow verification round of 71d19f4a83 (review2's 00:39Z ask), 2026-10-20 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `71d19f4a83`.
- **Method:** one brief (sha256 `bbe91e4d2f549ff46d41a8af14f3125e417e8ed349466e033cd3140c8f36ec0f`): round 3's findings verbatim, then "is each closed by 71d19f4a83, and did it break anything?". Shared by both reviewers, each blind to the other's review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** a new BLOCKER and 2 SHOULD-FIX, introduced by 71d19f4a83; round 3's findings closed or partly closed.
- **Disposition:** small fixes, in the commit named in the sign-off note (review2 reads them, no further round):
  - The retry loop: a hold's batch is applied quietly, so a refused hand-back's error batch isn't a commit that schedules another retry. A taken hold says its status (new BLOCKER).
  - A new pin clears a pending release (new SHOULD-FIX 1).
  - A copy's pin refused for its modes is remembered with the modes still held, so hand-back restores those (new SHOULD-FIX 2, on both pages).
  - The JS size check reads the live root size, not 16 (A3).
  - `app.mode` is cut to 800 bytes, so the whole `app` stays within 1 KiB (Grok B6).
- **Not taken:**
  - While a refused hand-back waits for its retry, the page shows the release before the runner has taken it (A2, B3). Only a refused commit can cause this; it is said as `unsupported` `pin.hold` and retried after each commit.
  - The page tests for refused hand-back, exponents and mixed commands (A6, B5): the runner test covers the transaction, and the paint after a release is tested.

**Blockers remain.** Reviewed `71d19f4a83` read-only; no builds or test suites run.

1. **A1 — CLOSED** — [host/web/src/host/mode.rs:72](/tmp/rv-dt-t7a7/host/web/src/host/mode.rs:72): successful takeover and partial-release receipts now reach the page.
2. **A2 — PARTLY** — [host/web/dev.js:575](/tmp/rv-dt-t7a7/host/web/dev.js:575): refusal reporting and retries were added, but CSS, `before`, `held`, and `app` still disappear before success; the revision is acknowledged.
3. **A3 — PARTLY** — [host/web-js/commands.js:49](/tmp/rv-dt-t7a7/host/web-js/commands.js:49): exponent grammar and numeric f32 validation are fixed, but percentages use hard-coded **16**, unlike the command’s actual host size; at 32px, `"2e39%"` wrongly replaces a valid remembered choice.
4. **A4 — CLOSED** — [host/web/dev.js:543](/tmp/rv-dt-t7a7/host/web/dev.js:543), [host/web-js/dev.mjs:714](/tmp/rv-dt-t7a7/host/web-js/dev.mjs:714): copy refusals retain previous mode holds and no longer restore the whole initial snapshot.
5. **A5 — CLOSED** — [host/web/dev.js:450](/tmp/rv-dt-t7a7/host/web/dev.js:450), [host/web-js/dev.mjs:708](/tmp/rv-dt-t7a7/host/web-js/dev.mjs:708): overlong mode entries are omitted, preventing the reported 65-character disagreement from rejecting status.
6. **A6 — PARTLY** — [scripts/dev-copies.test.mjs:193](/tmp/rv-dt-t7a7/scripts/dev-copies.test.mjs:193): adds WASM mode-paint and JS root-size assertions; WASM font delivery, refused hand-back, exponent percentages, mixed commands, and six simultaneous scrubs remain uncovered.
7. **B1 — CLOSED** — [host/web/src/host/mode.rs:72](/tmp/rv-dt-t7a7/host/web/src/host/mode.rs:72): non-null holds now deliver their commit receipts, covering all three dropped-receipt scenarios.
8. **B2 — CLOSED** — [host/web/dev.js:543](/tmp/rv-dt-t7a7/host/web/dev.js:543), [host/web-js/dev.mjs:714](/tmp/rv-dt-t7a7/host/web-js/dev.mjs:714): the original mode-release/whole-snapshot overwrite sequence is removed.
9. **B3 — PARTLY** — [host/web/dev.js:575](/tmp/rv-dt-t7a7/host/web/dev.js:575): the error is reported and retried, but page state still declares hand-back complete while the runner retains the hold.
10. **B4 — CLOSED** — [host/web-js/commands.js:49](/tmp/rv-dt-t7a7/host/web-js/commands.js:49): ordinary exponent percentages now pass through the hold hook; the remaining range mismatch is A3.
11. **B5 — PARTLY** — [scripts/dev-copies.test.mjs:178](/tmp/rv-dt-t7a7/scripts/dev-copies.test.mjs:178): partial-release paint is checked, but WASM still exercises no font pin or app font command.
12. **B6 — CLOSED** — [host/web/dev.js:450](/tmp/rv-dt-t7a7/host/web/dev.js:450): oversized mode entries are filtered and the mode object receives a byte bound; JS has the matching guard.

New issues introduced:

- **BLOCKER — Self-triggering retry loop:** [host/web/dev.js:478](/tmp/rv-dt-t7a7/host/web/dev.js:478) retries through `x.send`; even a refused batch invokes [`committed`](/tmp/rv-dt-t7a7/host/web/glue.js:900), scheduling another retry. A persistent refusal produces an endless timer/error loop without further app activity.
- **SHOULD-FIX — Pending release can undo a newer pin:** [`channel.releasing`](/tmp/rv-dt-t7a7/host/web/dev.js:575) is never cleared when accepting a replacement pin, so a subsequent retry can release that new hold.
- **SHOULD-FIX — Rejected pin replaces retained-hold metadata:** [WASM](/tmp/rv-dt-t7a7/host/web/dev.js:576) and [JS](/tmp/rv-dt-t7a7/host/web-js/dev.mjs:714) remember the rejected frame despite retaining old mode holds. Pin axis A, reject a replacement naming B, then hand back: without an app request for A, A stays at its pinned value because restoration consults B.
