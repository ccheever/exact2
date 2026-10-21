# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 16 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `5d54f487b6`.
- **Method:** one brief (sha256 `0a15e420c1be7f6b973992b8040188a9c163687075155fe25f1d2cfa6ff98fc6`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — a missing → unreadable `app.contract` transition bypasses the fix through cached params.**

[The cache reader](contract/cli/src/tamber_walk.rs:46) maps both `NotFound` and `PermissionDenied` to the same empty digest. [app_params()](contract/cli/src/tamber_walk.rs:257) therefore retains the missing-file result (`failed: false`, empty roots), even though the source graph is walked again.

Concrete reproduction, using the existing [resolved-package fixture](contract/cli/tests/it/tamber_serve_packages.rs:598), as a non-root user:

1. Keep its `vendor/p/package.json` containing `{}`, per-file links from `node_modules/p`, and `app.json` selecting `vendor/p/design.tamber`.
2. Temporarily remove `app.contract`. Start a fresh server and send `hello`, retaining `ok.files`. This caches `failed: false`.
3. Restore the fixture’s Contract containing `use P from "p"`, with mode `000`, before sending another request.
4. Send a transaction using those digests and `{"op":"set-token","name":"brand","value":"#654321"}`.

The transaction reaches the write to `vendor/p/design.tamber`. Its unnamed manifest and real path pass `in_package`; cached empty roots miss it; cached `failed: false` skips the [failed-load guard](contract/cli/src/tamber_packages.rs:145). The unreadable Contract also isn’t in the snapshot’s retained sources, because this server never loaded it.

This is the same clause-(c) package layout as the unreadable-Contract regression, with a cache transition added.

Source-traced reproduction; not executed because it requires filesystem writes. No files modified.
