# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ace12a15d`.
- **Method:** one brief (sha256 `c8cd1030634f8bebac2ef7aeee6064df63376789377e738e630dbe5ce056e6da`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; two should-fix: the hook and lag installed after the sender started; max(0.100, handoff + 0.050) lets a second hold pass at an 80 ms handoff.
- **Disposition:** taken in `bd89cc2f2`.

No BLOCKER findings. Two SHOULD-FIX issues:

1. **SHOULD-FIX — [MIDIMacTests.swift:358](/tmp/rv-ios-12/host/apple/tests/ExactKitTests/MIDIMacTests.swift:358): observer installation races with the drive.** The sender launches at line 345, before installation. Its first callback can miss the hook or race with assignment. The lock inside the closure protects `handed`, not the static closure itself; `nonisolated(unsafe)` adds no synchronization. Clearing it also lacks a callback-quiescence guarantee because shutdown disconnects asynchronously. **Fix:** install before launching the sender, synchronize publication/removal with readers or inject an immutable observer, and require a matching observation instead of silently using `?? 0`.

2. **SHOULD-FIX — [MIDIMacTests.swift:377](/tmp/rv-ios-12/host/apple/tests/ExactKitTests/MIDIMacTests.swift:377): an extra hold can still pass.** With an 80 ms handoff, the correct drain is due at 80 ms. An erroneous additional 50 ms hold delivers at 130 ms. A 10 ms main-lag sample anywhere in the measured interval raises the ceiling to 140 ms, so it passes—even if that lag never delayed the drain. **Fix:** retain this ceiling as a load-tolerant integration check, but test rejection of a second hold separately under controlled timing. The formula implements the requested bound; it cannot guarantee rejection in all cases.

Timestamp matching is sound for this fixture: JSON preserves fractional milliseconds, and ticks are approximately 20.8 ms apart. Exact duplicate stamps would overwrite earlier observations, but this drive does not generate them; broader reuse should preserve occurrence identity or the earliest observation.

With the hook permanently nil in production, the optional call skips argument evaluation, including `CACurrentMediaTime()`. There is no concurrent-write race in that case.

Read-only review; no builds or tests run. The load-80+ validation remains outstanding.
