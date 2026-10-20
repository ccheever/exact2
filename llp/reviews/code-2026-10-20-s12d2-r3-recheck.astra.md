# Code review: stage 12 D2, the slot check on writes, round 3 re-check (the blocker's fix, 1ebbb13aac) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `aa2d7f7a5b`.
- **Method:** one brief (sha256 `88f49d48792b4a3d67bd31cae657fa808c95aeeb156ac7e74973d9108672f050`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

- **Blocking:** None found.
- **Should-fix:** None.
- **Nits:** None.

The fix closes round 3’s blocker by inspection. The removed-name union in [tamber_scrub.rs:235](contract/cli/src/tamber_scrub.rs:235) also covers renames and moves out of the app design. Axis edits and `when` removal that change effective values remain covered by [changed_tokens:373](tamber/src/session.rs:373); parse failures select every chain. No remaining escape found in those cases.

The regression reproduces the blocker, and [the timing test:273](contract/cli/tests/it/tamber_serve_slots.rs:273) now reaches a chain through `lib.w`.

The permitted Cargo test was blocked by filesystem permissions before execution. No files changed.
