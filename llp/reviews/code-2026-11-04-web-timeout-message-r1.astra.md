# Code review: web timeout message, round 1, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `d80e264acc` (1 commit on `c22afd2d4c`).
- **Method:** one brief (sha256 `2dcf8d4109b92423c8394b49ed55bfce183efc4bf9a9c7fdf1e5d2bd9c565857`), one reviewer, pointed at the commit, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its source links name the review worktree's paths.

No findings in **d80e264acc against c22afd2d4c**. I read the governing documents, complete diff, relevant callers, and tests, and completed all four passes.

- **Adversarial correctness:** [admission.js:147](/tmp/exact-web-timeout-review-Qjqfk4/checkout/host/web-js/admission.js:147) correctly reports `until.ms`, the original timeout preserved by the file-body caller. The timer still uses the remaining duration. The guard is safe: the code clears `deadline` whenever it clears `until`. Caller-abort precedence and signal-owned deadlines remain unchanged.
- **Simplification:** The change reuses existing deadline metadata without adding state or another execution path. No simplification finding.
- **Writing:** The added comment explains the differing timeout values, and the commit message accurately describes the change. No writing finding.
- **Verification:** I executed the new [regression helper and assertion](/tmp/exact-web-timeout-review-Qjqfk4/checkout/host/web-js/abort-reason.test.mjs:67) in memory against both versions, retaining native timers and substituting an abort-aware transport. The parent failed with **30 ms**; the commit passed with **300 ms**. Additional controlled probes covered stalled body reads, expiry before sending, caller-versus-timeout races, signal-owned deadlines, valid and invalid timeout boundaries, network failures, and successful responses readable afterward. These passed.

Independent checks: `boot.mjs` and `git diff --check` passed. `caps.mjs` reported the existing navigation-file and recipe-size violations; their contents and cap definitions are byte-identical to the parent. Its additional wasm compilation check was blocked by sandbox write restrictions.

Verification limits: I did **not** run the complete repository test files or browser/IndexedDB integration suites; they require filesystem writes, builds, or browser fixtures unavailable under these restrictions. The only available author log, [build.log](/tmp/exact-web-timeout-checks-Ag3psb/build.log), records a completed Cargo build with linker warnings, but contains no commit identifier or JavaScript test results. I did not treat it as proof of the fix.

The checkout remained clean.