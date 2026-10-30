# Code review: the exact-js overlay cut, round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `8a08292664`.
- **Method:** one brief (sha256 `5e1f80dac6ca3b9c886e4de9a3a093402d0e2264c825712c3d1e510e06b6e921`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

M1 and M2 are fixed in `8a08292664`.

- **M1:** `load()` and `inspect()` share the installation/probe step at [js/src/lib.rs:605](js/src/lib.rs:605). Other inspection callers retain their metadata validation and bake restrictions.
- **M2:** Both getter reads occur under the guard at [js/src/prelude.js:1397](js/src/prelude.js:1397). Initialization remains outside it. Checks passed for pure getter initialization, `keep()`, and guard/context restoration after success, early return, and exceptions.
- The LLP worker qualification and QUEUE handshake follow-up accurately describe the current encoding path.

All **10 page-realm tests passed** with filesystem setup held in memory. The new getter test fails with the parent prelude. The native getter fixture also showed red/green behavior in a JavaScript VM. Replaying M1’s entry sequences produced HEAD’s matching `1 → 3` reads versus the parent inspection’s `0 → 2`.

**Validation limit:** the actual Rust/Hermes suite could not run because the read-only sandbox prevents build outputs. Native results above are VM checks, not native-suite results. No files changed.

VERDICT: APPROVE

No BLOCKER, MAJOR, MINOR, or NIT findings.
