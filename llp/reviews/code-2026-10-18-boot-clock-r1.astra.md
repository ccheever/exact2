# Code review: a fresh boot on a live clock starts the runner's clock at its reading (StoreRefreshTests), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `62b1d3eef`.
- **Method:** one brief (sha256 `1f7bb25943eadf02110fb25becd7a8b7e118901387bb2af89c2b45fb64b72175`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one should-fix (the test's 20000 thresholds no longer prove the refresh re-armed the timer) and a nit (the agents' guide on performanceNow's origin).
- **Disposition:** both taken in `cb1c4451b` (the refresh bracketed with the wall, a new deadline required for both readers; the guide says a window opened later reads the same clock).

Nothing is blocking. I found two nonblocking issues.

1. **SHOULD-FIX — [StoreRefreshTests.swift:125](/tmp/rv-ios-3/host/apple/tests/ExactKitTests/StoreRefreshTests.swift:125): the remaining `20000` thresholds no longer prove rearming.**  
   At process time 60000, the original short timer is already due at 70000. It satisfies both the batch filter and the later `> 20000` assertions even if the refresh incorrectly retains that deadline. **Fix:** capture each reader’s previous deadline and bracket the refresh; require the replacement deadline to fall within `[refreshing + 20000, refreshed + 20000]` and differ from the previous deadline.

2. **NIT — [contract-for-agents.md:2136](/tmp/rv-ios-3/docs/contract-for-agents.md:2136): clarify the clock’s origin.**  
   The guide promises a clock restarting at zero on every launch, while a fresh Apple session now observes process elapsed time during initialization. **Fix:** distinguish process-clock origin from session boot, explain that the initial reading can be nonzero, and retain the zero-start guarantee for fresh agent boots. Update the matching grammar, recipes, and diagnostic wording.

The runtime change looks correct by inspection. The launch reading reaches initializers, resource identity checks, timer bases, and virtual-frame bases before evaluation. `then` and queue scheduling establish their scheduling instant through `start_commit`. Validation rejects NaN, infinities, negatives, and values above `MAX_CLOCK_MS`; carried, checkpoint, and explicit `At` seeds retain precedence, including prepared candidates.

Apple’s host clock may already trail the runner by design: `engine_time` clamps it, and production motion waits for its first presented frame. Swift’s deadlines and `epochAtZero = Date − now()` remain in the same clock domain. Snapshot widget/activity boots should remain zero-based: snapshot admission rejects timers and `performanceNow()`, and callers supply the entry date separately. Fresh agent/replay boots remain deterministic at zero; journaling and checkpoints retain the actual runner clock.

Read-only review of `62b1d3eef9`; no builds or tests run.
