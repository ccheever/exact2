# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 20 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `39740510a7`.
- **Method:** one brief (sha256 `795ea6f3f24cf8860f0dc8da8b87d7c8a01d7d509d99fef891a28974423c8c2c`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** no blocking.
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

no blocking.**

A design-server write still cannot create or replace a package file under theme2's definition. Transactions (including proposal accept, scrub release, and a held revert or import) and undo all judge package files on a fresh load, and that judgment covers `.exact` as well as each file.

On the real path of the target, `read_only` refuses a file when any of these hold:

- A `package.json` above it, and below the app, names a package. Any read error other than a missing manifest refuses the write.
- The real path has a `node_modules` segment, or a link in a `node_modules` directory points at it.
- The path is under a package root the load resolved (direct, hoisted, vendored, transitive), unless that root holds the app.
- The load failed, and any `package.json` sits above the file under the app.

`.exact` is judged with that same check, including a contract-only write, because every `write=true` transaction calls `read_only` even when the op list is empty. Undo does the same before it applies an inverse. Temp files and the journal are only written after that check, and only into the `.exact` the server locked.
