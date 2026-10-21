# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 7 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f6bd98fe3d`.
- **Method:** one brief (sha256 `058536913dba1e7fcb91563e5747efadb2beed02e8c21174e765981e30557cb3`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — [tamber_packages.rs:321](contract/cli/src/tamber_packages.rs:321): the override transform discards required token kinds.** For a package token `reading.shadow: box-shadow = 0px 1px 4px #000`, `set-token` with value `0px 2px 8px #000` creates an untyped app line. Tamber explicitly [refuses untyped shadows](tamber/src/check.rs:706). Since `add-token` also refuses this package name, the promised override operation cannot set a supported package token. Transactions, proposals and scrubs share this failure. Retain or support the required kind while dropping package `whens`; add a typed-package regression.

Otherwise, inspection supports the provenance/digest fields, batch tracking, undo of transformed overrides, proposal revalidation, client `paths`/`restore` filtering, and the latest linked-directory ownership guard.

In-memory JS checks passed for baseline, changes, additions/deletions and private-path omission; changed JS files parse. Rust and filesystem integration tests were not run under the read-only constraint. No files changed.
