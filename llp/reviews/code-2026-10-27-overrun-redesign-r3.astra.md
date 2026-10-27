# Code review: the Overrun re-ask redesign, round 3, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `fa34d55045`.
- **Method:** one brief (sha256 `ae375e6fc58dcf4c5fbc90e4a05cf83ffa7310fe0f4e5acecd3b52f807edc8aa`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — Refresh ownership still disappears across a `Later` pass.** [settlement.rs:424](runner/src/runner/settlement.rs:424). An existing wait is refreshed, the first pass returns `Later`, a later resource writes the store, and the second pass overruns. `refreshed` recognizes only a preceding `Wait`, so it becomes false. The Overrun recovers the old streak, and `hold_reask` preserves the old ticket, deadline and invocation. An App-owned wait refreshed by a tool can therefore retry as App and escape the tool’s ownership. Preserve refresh metadata across both `Later` and `Wait`, including when publishing a replacement `Later`.

- **SHOULD — The reorder filter reopens a deadline-ordering hole.** [advance.rs:317](runner/src/runner/advance.rs:317). With overdue deadlines of hold=1000, re-ask=1250 and `then`=1300, the hold filters `reask` to `None`. Consequently, `ready(1300)` succeeds and the `then` executes before both earlier deadlines. It can supersede the due re-ask or reconcile the drop before its timeout. Keep the earliest re-ask in the ordering checks even while an earlier hold prevents dispatch.

Reviewed `d3a58d5e2a..fa34d55045` read-only. Findings are source-traced; tests were not run because this checkout has no build artifacts and the sandbox prohibits writes.
