# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 8 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f19a866988`.
- **Method:** one brief (sha256 `42b483eca7facddf9affb5176bfb22de62d2f50541def0af2463867be2a026c9`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — Undo bypasses package read-only enforcement.** The guard at [tamber_serve.rs:1107](contract/cli/src/tamber_serve.rs:1107) covers transactions/proposals, but [undo:1174](contract/cli/src/tamber_serve.rs:1174) calls `run` directly.

Reproducer: configure `theme/colors.tamber` as the app design, edit it, then link `node_modules/@acme/theme` to `theme` without changing the design’s bytes. Normal writes now refuse it as package-owned; the saved inverse still passes its digest check and writes it. Recheck package ownership before applying inverses. The existing undo test never changes ownership.

Otherwise, the reviewed paths check out: typed overrides preserve kind; whole-token/`when` behavior and batch tracking are consistent; proposals and scrubs share the transform; package metadata, digest announcements, refresh ordering, and client path stripping match the design.

File-free tracker checks passed for baseline, changes, additions/deletions, and path stripping. Rust/integration tests were inspected but not run because they create files. Repository unchanged.
