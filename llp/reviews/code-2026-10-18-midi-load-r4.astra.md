# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 4, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ce32fa972`.
- **Method:** one brief (sha256 `1d5c104c1b2e43a53ec3d71a0580935f0ab1c2548e7a58fc59c0612af8684579`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; two should-fix: Start could go unchecked silently; a qualified send could still fail on delivery jitter.
- **Disposition:** taken in `067dc3c86` (Start's own gate and skip; a 20 ms send ceiling; and a load gate, after a qualified send failed at load 97).

Nothing blocking found in `ce32fa9720`. Two nonblocking findings:

1. **SHOULD-FIX — [MIDIMacTests.swift:414](/tmp/rv-ios-14/host/apple/tests/ExactKitTests/MIDIMacTests.swift:414): Start can go unchecked while the test passes.** A regressed Start arriving 50 ms late escapes its assertion if one main-queue beat in that window was 3 ms late. Three qualifying tick batches still let the test pass without reporting omitted Start coverage. **Fix:** track Start qualification separately, retry it, and explicitly skip with a reason if no attempt qualifies.

2. **SHOULD-FIX — [MIDIMacTests.swift:147](/tmp/rv-ios-14/host/apple/tests/ExactKitTests/MIDIMacTests.swift:147): send qualification still permits load-related false failures.** Correct code whose destination callback is delayed 5 ms can qualify under the 20 ms probe threshold, then fail the fixed 2 ms assertion. Setting `timed = true` ends retries immediately. The separate queue’s 10 ms sampling cannot exclude such short delivery stalls. **Fix:** distinguish the arm’s 50 ms handoff margin from final delivery jitter; tighten and validate the delivery qualifier, retrying inconclusive attempts while retaining the fixed ceiling.

The weak capture and locked observer snapshot address the reported lifetime and data-race issues. The observer executes outside its accessor lock. The previous widened-ceiling counterexamples no longer pass a qualified timing assertion.

Skip frequency across load **20–60 remains unverified**: the reported three runs near 25 support that narrower condition. This was a read-only review; no builds, tests, or file/index changes.
