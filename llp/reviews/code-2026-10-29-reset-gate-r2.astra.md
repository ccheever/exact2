# Code review: ResetGate's tables after #1141 and #1144, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `60fe90927d`.
- **Method:** one brief (sha256 `65d8b161630c7cfed021804f2394a3f67e2ab00c00166d05799ea651213b2a80`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [ResetGateTests.swift:336](host/apple/tests/ExactKitTests/ResetGateTests.swift:336), also line 367: `window` is a `UIWindow` on iOS, which has no `close()` method. Both new tests fail to compile there. Use the same platform-specific cleanup as `tearDown`: hide on iOS, close on macOS.

2. [reset_gate.rs:28](host/apple/tests/it/reset_gate.rs:28): `wipe` does not mutate captured state, so its binding’s `mut` triggers `unused_mut`. The required Clippy invocation uses `-D warnings`, making this a gate failure even though the Rust tests pass. Remove `mut`.

3. [ResetGate.swift:130](host/apple/Sources/ExactKit/ResetGate.swift:130): `deliverySync()`’s recovery reason covers only `applyTogether`. The dev-menu fresh boot (`finishBoot`) and direct `ExactSession.apply` also reset, but neither repeats `deliverySync`. A refresh held during either path can therefore leave delivery facts stale. Keep this in `knownLoss`, or refresh delivery in the common reset epilogue.

**Non-blocking**

1. [reset_gate.rs:187](host/apple/tests/it/reset_gate.rs:187): named holds are still scanned inside string contents. A multiline Swift string containing `runtime.holds("obsolete()")` keeps that stale key alive. Separately, `gate_keys` reads raw text, so dictionary entries inside block comments still count. Locate calls using the string-blanked mask, extract their arguments from the original text, and strip comments before extracting table keys. Add fixtures for both cases.

2. [ResetGateTests.swift:325](host/apple/tests/ExactKitTests/ResetGateTests.swift:325): eventual `ticks > before` does not prove the held firing survived—the fixture repeats every 50 ms, so a later firing can satisfy it. The preceding polling loop also does not assert that its condition succeeded before timeout. Use a one-shot firing or prevent subsequent firings, assert the retained queue state, and require exactly one increment.

The other reported round-1 fixes are addressed in the inspected code. Both diffs reviewed; no tests or builds run.
