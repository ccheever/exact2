# Include the separate game workspace in asynchronous verification

**Status:** Open
**Systems:** game workspace, shared GPU API, async lane
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** existing game Frame and lock issues; scripts/async.mjs

The game workspace consumes shared exact2 crates but is not part of the root workspace/default-members. The current async lane's --workspace command still refers only to the root workspace; it never selects game/Cargo.toml.

The current review's `cargo test --manifest-path game/Cargo.toml --workspace --lib --bins --tests --no-fail-fast` fails to compile the look test because `exact_gpu::Frame` gained headroom. A locked cargo tree also refuses the stale game lock. Both are filed separately by the parallel review as `20261006-game-frame-literals-missing-headroom.md` and `20261006-game-lock-missing-sha2.md`. Root build, clippy and formatting passed while these consumers were broken.

Extend the existing asynchronous lane to build/test the separate workspace with a frozen lock when game files or any of its shared dependency inputs change. Use the dependency/build-input model already available, or run this workspace conservatively. Keep expensive GPU/display execution in an appropriate existing asynchronous tier; this requires no new blocking check.

Acceptance: a shared Frame/API change triggers a game-workspace compile/test result and catches a missing field; stale lockfiles fail without rewriting them. Check relevant game library/tests and distinguish device-dependent tests from source compilation. The fixed issues cannot silently recur under a green root-only result.
