# Code review: the Contract compiler's quadratic growth, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6d6c847c1c`.
- **Method:** one brief (sha256 `b00c529de40cac8d129fca1f817344e8127f3d44b01bc70585c44a90acadbd18`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **[P2] Growth test breaks Windows test compilation** — [growth.rs:51](contract/cli/tests/growth.rs:51). `cpu()` references `libc` unconditionally, while [Cargo.toml:34](contract/cli/Cargo.toml:34) declares it only under `cfg(unix)`. On Windows, `cargo test -p contract --test growth --no-run` fails with unresolved `libc`. Gate the test to supported platforms or provide platform-specific timing.

2. **[P2] The change fails the source-line cap** — [lower/src/lib.rs:411](contract/lower/src/lib.rs:411). Caps reports **1,508 lines**, exceeding 1,500. This violation is introduced by these commits, independently of the existing Swift violation. Move the map construction into another module and rerun caps.

Non-blocking notes:

- [calls.rs:399](contract/types/src/calls.rs:399): `refined` records a callee once per changed parameter, then clones its entire parameter vector for every entry. Refining *p* parameters introduces O(p²) copying. Deduplicating callee indices avoids this; the growth fixture’s single-parameter callee cannot expose it.
- [growth.rs:103](contract/cli/tests/growth.rs:103): 4.6 leaves about 7% headroom above the reported clean maximum and only 4% separation from the weakest reported regression. Thread CPU timing helps, but cross-machine calibration remains advisable.
- I found no semantic mismatch in first-wins behavior, shadowing, state/mutation precedence, scope refresh ordering, sharing, or tint aggregation. The revised shadowing assertions and refresh property test exercise meaningful differences.

Formatting and `git diff --check` passed. Caps failed as described, with its wasm compilation skipped. The sandbox rejected creation of the requested `/tmp` target, preventing independent test/clippy runs and byte-parity reproduction. The checkout remains unchanged.

CHANGES
