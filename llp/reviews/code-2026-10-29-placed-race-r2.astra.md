# Code review: the Placed retirement race, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `59345f0a35`.
- **Method:** one brief (sha256 `0e999edcfb4ee89343dd41ece7fe81c811742d6e95a35df14c06276107ddc35b`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [data/src/placed.rs:771](data/src/placed.rs:771) — **A live turn can lose its token through a wrapper.** [Storage::dispatch](data/host/src/lib.rs:338) removes its token mapping when dispatch returns `Run`; subsequent retirements forward `continuation: None`. This also propagates through Fieldnotes’ `Storage<Mixed<Placed<…>>>` composition. If an unrelated retirement arrives before hand-off, `unqueued.retain` deletes the live token, and its closure returns `Aborted`. The runner still holds that ticket, so this fails a wanted call. Preserve dispatched-token translations through retirement, or retain enough identity to honor a live key whose token is unknown. Add a delayed-hand-off regression through the actual wrappers.

2. [data/src/placed.rs:771](data/src/placed.rs:771) — **Untargeted tool-handler turns are incorrectly cancelled.** [run_handler](runner/src/runner/modules/invoke.rs:258) calls `DataSource::answer`, producing `target: None`; its ticket lives in `invocations.running`. [hear_forgotten](runner/src/runner/commit.rs:234) lists only targeted pending requests. Consequently, any unrelated retirement between dispatch and hand-off removes the handler’s token despite its ticket remaining live. Track the target alongside each unqueued token and exempt untargeted calls, consistent with the existing stage retention and `forget_in_flight` rules. Test an untargeted hand-off across an unrelated retirement.

**Non-blocking**

1. [data/src/placed.rs:805](data/src/placed.rs:805) — **An unrun closure can leave a stale token.** Dropping `Work::Later` does not remove its entry. Ordinary targeted admission failures eventually trigger retirement cleanup, but a refused tool-handler request finishes through [tool_landed](runner/src/runner/modules/invoke.rs:409) without that cleanup. Repeated refusals can accumulate token entries until another retirement or proxy destruction. A captured drop guard should remove abandoned entries.

Round 1’s original ordering race is resolved. The intersection and lock order look sound: the owner releases the slot mutex before calling the source and never acquires the queue mutex. Timer waits remain in retirement snapshots, rollback restores pending requests before notification, and validation runs inline. I found no separate regression involving Resume turns or streams.

Static review only; no edits, tests, builds, or network access.
