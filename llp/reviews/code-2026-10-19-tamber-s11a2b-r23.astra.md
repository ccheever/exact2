# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 20 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `39740510a7`.
- **Method:** one brief (sha256 `795ea6f3f24cf8860f0dc8da8b87d7c8a01d7d509d99fef891a28974423c8c2c`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — the wire can write a package file before the Rust guard runs.**

[`write()`](scripts/tamber-server.mjs:856) calls `session.prepare()`, which runs plain [`git status`](scripts/tamber-commit.mjs:74). Git refreshes and writes its index by default; this happens before the request reaches `read_only`.

Concrete reproduction:

1. Create a valid app with tracked `app.tamber`, a Git commit, and Git’s separate directory at `app/node_modules/git-meta`.
2. Start the wire with `GIT_OPTIONAL_LOCKS` unset. Change only `app.tamber`’s mtime so its cached index metadata is stale.
3. Send a normal `transaction` setting a token, with the current design digest.
4. The preparatory `git status` rewrites `app/node_modules/git-meta/index`. Its real path contains `node_modules`, so this is a package-file write under **(b)**. Neither the design-file guard nor the `.exact` guard covers it.

Disable optional index writes for this preparatory read (`git --no-optional-locks status`).

Read-only validation: I exercised `prepare()` with an in-memory subprocess stub and confirmed Git’s behavior in its installed documentation. I did not execute the filesystem-changing reproduction.
