# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 6 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `9f15631b41`.
- **Method:** one brief (sha256 `fb9ecf97db0cb584d1bb135537165c2e0f1a90023a774b70b36c0913d7cde55c`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** no blocking.
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

No BLOCKING issues. Re-checked HEAD `9f15631` for security, read/write correctness, and data loss. The earlier watcher gaps stay closed, and nothing new in that class corrupts a read, a write, or a package file.

What holds:

- **Reads.** With no package designs, `hello` / `describe` / `resolve` / `explain` add no fields. Package rows carry `package`, `file`, and a text digest; `resolve` has no `package_value`; `explain` adds it only for an app-set token, listing the package `when`s the override drops. The walk key includes those design digests, so a request re-reads them.
- **Writes.** `set-token` of a name the app does not define (tracked through the batch) but a package or a base-scale name does becomes an `add-token` of the value only, on the app file. A `when` on that first set is refused. `add-token` of a package or base name is refused unless the server set `restore`. Rename onto a package name, a base name, or a package namespace is refused. `remove-token` drops the app line. Scrub-fold uses the same transform. Undo stores the inverse of the transformed op (`Restore` of the added line), so it does not pass through the transform again and puts the token back on the package.
- **Restore is server-owned.** The wire deletes a client's `restore` on every method. Held revert/import proposals, and a local `propose-hold` with `review.importer`, keep `restoring` and send it on accept. A client `transaction` with `restore: true` cannot take that path.
- **Read-only.** An op whose file is a package design (wire name, path, or canonical path) or any existing file under a named `package.json` below the app, or under `node_modules`, is refused before plan, including when the app's load fails. `plan` still rejects a path that is not one of the design's files, and commit renames only those snapshot paths.
- **Remote hello.** A client's hello is sent with `paths: false`, so it gets neither `path` nor `watch`. The `changed` event's `packages` entries are `{package, file, digest}` only. `watch.links` stays inside the app; roots are package directories (including a linked package outside the app), not a `package.json` `"tamber"` path outside its package.
- **Watcher vs data.** Listing is seeded when a directory is first listed; install paths are looked at too; a directory listed once stays watched while it exists. A missed push only delays `changed`. Accept and release run the transform again on the current packages, and a proposal planned on a token the package then drops is refused rather than written.
