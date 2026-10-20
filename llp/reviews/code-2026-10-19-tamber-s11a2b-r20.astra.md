# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 17 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `2b20d30ac1`.
- **Method:** one brief (sha256 `44948e2f607924fd1c42bb37ff3731859a0fe04cbf523b884e25afcd988dd5a1`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

BLOCKING — a named package at `.exact/` receives unguarded transaction writes.

Concrete reproduction:

1. Create an ordinary app with `app.tamber` containing:
   ```text
   tamber t

   theme
     ink = #111111
   ```
2. Put `{"name":"p"}` in `app/.exact/package.json`, using real directories.
3. Start the design server, call `hello`, then submit a transaction setting `ink` to `#222222`, supplying the returned file digests.

The guard checks the operation’s `app.tamber` target. The commit then writes `.exact/tamber-*.tmp` and `.exact/tamber.journal.tmp` without checking their package ownership: [tamber_serve.rs:1160](contract/cli/src/tamber_serve.rs:1160), [tamber_serve.rs:1189](contract/cli/src/tamber_serve.rs:1189).

Those files satisfy **(a)**: their real paths are beneath `.exact`, whose manifest names a package. These are ordinary commit writes; crash recovery is unnecessary.

This reproduction is source-traced, not executed, to preserve your read-only constraint.
