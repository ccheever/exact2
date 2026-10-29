# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 4, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `a581a720f0`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Both reported races are fixed in this increment. **Two blockers remain in the whole change.**

1. **[P1] Keep-two can lose a later abort of a still-running POST.** Send the same mutation/source/arguments three times, retaining each controller, then abort the first. The third fetch deletes the first mapping at [js/web/src/lib.rs:515](js/web/src/lib.rs:515). This is reachable: the realm overwrites its pending entry without releasing the preceding call ([module-glue.js:241](host/web/module-glue.js:241)), and same-key replacement survives the module’s key-based retention ([js/web/src/lib.rs:925](js/web/src/lib.rs:925)). Superseded POSTs continue running ([glue.js:740](host/web/glue.js:740)). The later abort enters `unmapped`, but that deleted mapping will never be recorded again, so no cancellation reaches the host.

2. **[P1] A discarded worker reply can strand another request’s abort.** A worker turn can remain inside an awaited storage operation after its runner ticket is forgotten, then abort another independent fetch. Its normal completion publishes the abort but calls no `abort_wake` ([data/src/placed.rs:423](data/src/placed.rs:423)). If another ordered request remains outstanding, the executor discards that forgotten turn’s reply without waking ([executor_core.rs:951](host/apple/src/executor_core.rs:951)). The subsequent `Forgotten` job finds no fresh abort—the preceding turn already drained it—so its conditional wake does not help ([placed.rs:375](data/src/placed.rs:375)). Cancellation waits for unrelated input or completion. Wake whenever a nonempty abort batch is published.

For **(a)**, fetch registration now precedes polling and pruning ([js/web/src/lib.rs:507](js/web/src/lib.rs:507)); retries also run when querying the realm fails ([lib.rs:216](js/web/src/lib.rs:216)). The runner fix also holds: retired matches enter `module_aborts` ([admission.rs:135](runner/src/runner/admission.rs:135)); rollback restores `pending` ([commit.rs:180](runner/src/runner/commit.rs:180)); the pump then generates `Failed { Aborted }` plus cancellation for the restored ticket ([admission.rs:202](runner/src/runner/admission.rs:202)). Without rollback, it skips the answer and still cancels, including Linux’s empty-pump path ([pump.rs:42](host/linux/src/presenter/pump.rs:42)).

For **(b)**, I found no client-reachable ticket reuse or cross-key misattribution: tickets increase per realm ([prelude.js:677](js/src/prelude.js:677)), and replacement constructs fresh module bookkeeping ([lib.rs:779](js/web/src/lib.rs:779)). The mapping-loss case above remains.

For **(c–d)**, I found no self-sustaining wake loop or additional regression specific to this increment; the worker wake omission is the remaining native stranding path.

Static review only; no builds or tests run, and no files changed.

Verdict: blockers — P1 keep-two mapping loss; P1 missing wake after a discarded worker reply.
