# Code review: the knownLoss follow-up (ResetGate tables, ResetHoldTests), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `826308a57a`.
- **Method:** one brief (sha256 `fbfa80b153a5b41ebc2f253b12ccc6c81fc9679a232f593957d6453278bcc4a5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [ResetHoldTests.swift:23](host/apple/tests/ExactKitTests/ResetHoldTests.swift:23) — **iOS can pass without exercising sheet dismissal.** Without `missing("scene", …)`, `xctest.mjs` runs this in the bare agent. `presentedViewController != nil` does not prove presentation completed; `DialogSheetIOS.dismiss` explicitly defers dismissal while presentation is unfinished. The iOS branch never asserts `after == false`. Require a hosted scene, wait for presentation to finish, and verify the restart dismissed the sheet.

2. [ResetHoldTests.swift:57](host/apple/tests/ExactKitTests/ResetHoldTests.swift:57), [ResetGate.swift:155](host/apple/Sources/ExactKit/ResetGate.swift:155), [QUEUE.md:4](QUEUE.md:4) — **The probes do not establish “no nested run loop.”** `.common` covers selected modes, not every mode; another non-common mode could run without servicing these probes. The single-window test adds two explicit modes, but never proves that probe fires afterward; it accepts any one callback, while the two-window test requires none afterward. Add per-probe positive controls in the relevant modes for both tests, and narrow the prose to the observed delivery behavior unless broader loop detection is added. Queue probes also do not establish the blanket conclusion for synchronous callers.

3. [ResetHoldTests.swift:64](host/apple/tests/ExactKitTests/ResetHoldTests.swift:64) — **The Mac timing guard measures all of `apply`, not `endSheet`.** Preparation, commit, or rebuilding the presentation could exceed 0.15 seconds even with instantaneous sheet teardown. The two-window variant has no timing guard. Use a test window subclass that times `super.endSheet`, and assert the required duration for each sheet.

**Non-blocking**

none.

The three moved entries are moot as documented: generation guards reject the old surface/region completions, and surface records are not carried. The `deliverySync` qualifier is correct. Swift `knownLoss` and Rust `KNOWN_LOSS` match exactly: **16 entries**.

No tests or builds were run.
