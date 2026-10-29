# Code review: four Mac reds as test drift (main red), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5c151ea331`.
- **Method:** one brief (sha256 `0842e466aa915f75b8dbd8323e2bf3eab739e83f8589034679b4efd049a34141`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

None. The four classes’ failures are consistent with test drift:

- **MacShortcutTests:** #1141 adds three commands without shortcuts. The rewritten assertions retain the existing chords and modifiers.
- **StoreRefreshTests:** #1144 moves ordinary task deadlines into native timer rows. The assertions still require boot-relative scheduling, a timer-only refresh batch, and both readers adopting the rearmed deadline.
- **AsyncAdvanceTests:** The held-publication window remains real: the presenter still contains the removed row while owner state has changed. Refusal/logging and current-state input assertions remain intact. The delayed test blocks the owner before submission and still checks execution-time sampling, one coalesced beat, and the original 40 ms phase.
- **LiveClockMacTests:** `fireNativeTimer` exercises the native runtime entry. The subsequent virtual advance’s total of four checks that missed live beats were skipped and three virtual beats execute. It does not independently test Foundation’s scheduling; `NativeTaskTimersTests` covers the adapter.

`deliverTask` has no competing Foundation firing in this window: timers run on `RunLoop.main`, and these synchronous tests neither pump that loop during `Thread.sleep` nor through `Owner.sync`. It invokes the actual timer callback and production delivery method.

The shared changes are compatible with the configured iOS **Debug** test build: `NativeTimerWake` is public, the timer implementation is shared, and `timerForTests` is available under `DEBUG`. This is source inspection, not a verified build.

**Non-blocking**

1. [AsyncAdvanceTests.swift:191](host/apple/tests/ExactKitTests/AsyncAdvanceTests.swift:191): The name and comments still claim to test a due timer being posted by batch landing through `advanceIfDue`. The rewritten test explicitly delivers a native wake instead. Rename it to describe native wake posting and update the related `advanceIfDue` comments in the delayed test.

No files edited; no network, tests, or builds run.
