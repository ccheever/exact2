# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 6, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `b4cc675603`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**One blocker remains: worker placement can leave a superseded native fetch abortable.** This is a source-level finding; no builds or tests were run.

**(a) Dedupe is correct.** [admission.rs:161](runner/src/runner/admission.rs:161) checks the runner ticket before eviction or insertion. Repeated retirement therefore leaves one retained record. [abort_tests.rs:139](runner/src/runner/abort_tests.rs:139) exercises exactly that helper 300 times; it does not execute 300 refused commits.

**(b) The native argument holds once the call reaches `__exact_forget`, but not on every worker path.** The normal chain is present: runner [hear_forgotten](runner/src/runner/commit.rs:216) → [Module::forgotten](js/src/source.rs:247) → [forget_calls](js/src/turns.rs:71). The prelude [settles owned fetches](js/src/prelude.js:1295), and [settled releases their subscriptions](js/src/prelude.js:504).

However, this worker interleaving is allowed:

1. A pending POST **A** is replaced by **B** with the same target/source/arguments. While B remains recorded, [Placed::forgotten excludes its key](data/src/placed.rs:799), correctly arranging to forget A.
2. [Placed::dispatch removes B’s record immediately](data/src/placed.rs:817), but queues its owner turn only when the returned [Work::Later executes](data/src/placed.rs:843). The native executor [queues that work](host/apple/src/executor_core.rs:602).
3. With the owner busy and that handoff delayed, another retirement can arrive. B is no longer recorded, so its key now passes the filter. [OwnerQueue::forgotten overwrites the still-pending snapshot](data/src/placed.rs:61); the coalescing barrier is established only by [OwnerQueue::turn](data/src/placed.rs:75).
4. `Job::Forgotten` then forwards the key with [continuation: None](data/src/placed.rs:458). [Module::forget_in_flight keeps matching calls in that case](js/src/turns.rs:125), so A never reaches `__exact_forget`.

This can persist: if B waits on a timer, [Module::begin preserves older calls under that key](js/src/lib.rs:1068), and Placed’s [recorded timer resume](data/src/placed.rs:337) is not excluded by the `Recorded::Answer` filter. A remains abortable in later turns.

Then 256 unrelated, sequential fetch retirements can evict A’s mapping: native [forgotten completions are dropped](host/apple/src/executor_core.rs:951), while [the cap removes the oldest record](runner/src/runner/admission.rs:166). A subsequent abort [matches neither pending nor retired](runner/src/runner/admission.rs:118), leaving its POST running. This does not require 257 simultaneously live superseded requests.

**(c) I accept the explicit web 256-record bound.** HTTP completion calls [safelyFulfill](host/web/glue.js:756), reaches [runner.fulfill](host/web/src/host.rs:1234), and [removes the record before checking pending](runner/src/runner/commit.rs:1187). Precisely, the cap counts retirement records—including still-live requests restored by refusals—not exclusively currently superseded requests.

**(d) No additional issue found in the incremental dedupe change.** The worker gap above invalidates its new native-safety explanation.

Verdict: blocker — worker retirement can preserve an abortable superseded fetch until stale native records evict its cancellation mapping.
