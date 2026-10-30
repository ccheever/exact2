# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 5, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `71d8099659`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Both re-check-4 cases are fixed, but **one blocker remains: the retired-record cap can discard a live request’s abort mapping**. This was static inspection only; no builds, tests, or edits.

**(a) Both specific fixes are present.**

- The three-send case no longer depends on a module’s keep-two table. The realm returns tickets directly, and the runner matches pending or retired requests. Retired entries no longer age by collections. See [js/web/src/lib.rs:792](js/web/src/lib.rs:792) and [admission.rs:113](runner/src/runner/admission.rs:113).
- `publish_aborts` publishes before waking, ignores empty batches, and is called after both ordinary turns and let-go turns—even if their reply is discarded. See [placed.rs:87](data/src/placed.rs:87), [placed.rs:395](data/src/placed.rs:395), and [placed.rs:430](data/src/placed.rs:430).

**(b) I found no wrong-request match on the traced in-tree client paths.**

| Case | Finding |
|---|---|
| Engine restart/unload | A literal unload/reload inside the same runner would be unsafe: the engine restarts its counter at 1, while runner records survive. However, I found no such production client path. `load` is idempotent; validation unloads a replacement candidate; web replacement boots a new runner. See [lib.rs:511](js/src/lib.rs:511), [prelude.js:435](js/src/prelude.js:435), [abi.rs:705](host/apple/src/abi.rs:705), and [web abi.rs:272](host/web/src/abi.rs:272). |
| Mixed with Rust | Safe for the in-tree composition: Rust requests carry `fetch: None`, including decoded module requests. See [request.rs:328](runner/src/request.rs:328) and [logic/abi/src/lib.rs:501](logic/abi/src/lib.rs:501). |
| Continuation or stream with the same number | Matching tests `fetch`, excludes continuations and streams, and the prelude emits no door-22 abort for streams. See [admission.rs:120](runner/src/runner/admission.rs:120) and [prelude.js:563](js/src/prelude.js:563). |
| Untargeted invocation | Its fetch does not acquire another request’s identity. It remains unmatched because tool requests live outside `pending`; cancellation remains the documented exception. See [invoke.rs:338](runner/src/runner/modules/invoke.rs:338) and [reference.md:1045](docs/reference.md:1045). |
| Refused commit | The abort retains the runner ticket, and the pump checks whether that ticket is pending after rollback. That avoids misdelivery. Retirement bookkeeping has a separate defect below. See [admission.rs:188](runner/src/runner/admission.rs:188). |

**(c) Retention is bounded, but eviction is incorrect.**

`note_retired` unconditionally removes the oldest entry at 256 and does not deduplicate. Completion cleanup happens only in `fulfill_measured`. See [admission.rs:157](runner/src/runner/admission.rs:157) and [commit.rs:1186](runner/src/runner/commit.rs:1186).

A concrete wasm trace needs only two outstanding POSTs:

1. Send A, then replace it with B on the same source/arguments. A remains running with a retired mapping.
2. Repeatedly assign B’s mutation slot in an action whose settlement refuses. Each attempt records B as retired **before** settlement; rollback restores B to `pending` without undoing that record. See [commit.rs:571](runner/src/runner/commit.rs:571) and [commit.rs:180](runner/src/runner/commit.rs:180).
3. After 256 such refusals, duplicate B records evict A.
4. Aborting A now matches neither table, so no transport cancellation is issued.

This is reachable through the wasm realm: successive calls overwrite the same parked key, and superseded POSTs continue running. See [module-glue.js:241](host/web/module-glue.js:241) and [glue.js:740](host/web/glue.js:740). More than 256 genuinely outstanding retired requests also loses mappings without rollback.

Apple and Linux additionally retain stale records: forgotten completions are discarded before reaching `fulfill_measured`. Both executor implementations do this, and Linux shares the core. See [executor_core.rs:951](host/apple/src/executor_core.rs:951), [executor_async.rs:1098](host/apple/src/executor_async.rs:1098), and [linux/executor.rs:8](host/linux/src/executor.rs:8). This is bounded retention, but the assumed completion cleanup does not occur there.

**(d) No additional wake loop found.**

Publication releases its locks before waking; consumption drains the batch; executor wakes coalesce. Unmatched `heard_aborts` do not themselves keep pumping. See [placed.rs:95](data/src/placed.rs:95), [placed.rs:563](data/src/placed.rs:563), [executor_core.rs:1005](host/apple/src/executor_core.rs:1005), and [admission.rs:173](runner/src/runner/admission.rs:173).

Verdict: blocker — retired-record eviction can leave a live wasm POST uncancellable, including through duplicate records created by rollback.
