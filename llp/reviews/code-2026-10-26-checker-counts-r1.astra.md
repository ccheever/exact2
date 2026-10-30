# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), round 1 of 3, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `f705da0591`.
- **Method:** one brief (sha256 `1df957552a166848609ed095bcaca8256f9ca5b1be224d807529c8cc06ef8446`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [check.rs:1126](tamber/src/check.rs:1126): probe order changes answers.** `reach` uses LIFO discovery order. Repro: one number dial `d`; 512 explicitly typed number tokens, with `t0 = d`, `t1` through `t510` aliasing their predecessor, and `t511 = calc(t510 + d)`. The old declaration-order probes succeed using memoized predecessors. The new probe starts at `t511`, exceeds the 512-level bound, and memoizes `None` throughout the chain. All influence disappears, producing `tamber-dial-inert`. The `every_token` toggle also uses this reordered traversal, so it misses the regression. Preserve fold order and compare against the actual old full-refold path.

- **BLOCKER — [export_batch.rs:65](tamber/src/export_batch.rs:65): ordinary shared-dial imports exhaust the budget.** Consider 1,000 tokens named `surface.component0` … `surface.component999`, all reading one color dial. Change every exported value from black to white. `rounds()` trials each candidate separately, charging the approximately 31,939-byte design 1,000 times: **31.9 million units**, exceeding `REIMPORT`’s 25,165,824 before other charges. This input fits the wire limit and ultimately needs one `SetDial`, with no refused edits. Avoid separately validating every literal fallback before determining whether the shared dial accepts the change.

- **BLOCKER, remaining pre-existing path — [tamber_ops.rs:196](contract/cli/src/tamber_ops.rs:196): wire parameter writes retain the expensive nested scan.** Each operation searches existing parameter entries using `resolve()`, which rebuilds and scans the entire parameter list. Repeating 1,024 writes to a parameter near the end costs operations × entries × parameters before reaching the improved `laid()`. This runs through the shared serve endpoint. `entry_errors()` also retains repeated resolution; `written_ranges()` additionally refolds the whole design per ranged value. The isolated `laid()` timing test does not cover these paths.

- **SHOULD — [export.rs:654](tamber/src/export.rs:654): exhaustion does not stop the remaining work.** `take()` silently returns, while callers continue singleton rounds, cloning `cur` before the next charge, then execute uncharged dial checks and the final base check. The wrapper eventually discards the proposal, but substantial—and potentially quadratic—work continues after exhaustion. Propagate exhaustion immediately through planning and batch callers.

- **SHOULD — [check.rs:1070](tamber/src/check.rs:1070): influence still initializes every token per dial.** Allocating and clearing `moving` inside the dial loop retains Θ(dials × tokens) work even with disjoint readers and linear output. Allocate visitation storage once and use generation markers or reset only visited entries.

I found no additional mismatch in the parameter index or memo restoration, and no budget-state leak into a later top-level import. The wrapper prevents exhausted proposals from escaping as operations.

Rust tests could not run in this read-only checkout without build artifacts. In-memory models confirmed the traversal counterexample and budget arithmetic. No files changed.
