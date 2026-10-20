# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 8 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `f19a866988`.
- **Method:** one brief (sha256 `42b483eca7facddf9affb5176bfb22de62d2f50541def0af2463867be2a026c9`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** no blocking.
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

No BLOCKING findings.

Re-checked `f19a866988` for security, read/write correctness, and data loss. The re-check 7 kind fix holds: a `set-token` override of a package token the app does not yet define is written as an `add-token` that keeps the package line's written kind (`tamber_packages.rs:319-329`), so a shadow stays `box-shadow`. Base names with no package owner stay untyped (`space.md = 12px`). A later `when` in the same batch edits that new line. Undo is a byte restore of the lines the edit replaced, and `remove-token` drops the override so the package line applies again.

What checks out:

- Package files stay read-only by wire name, path, and canonical identity, and by `in_package` even when the app's load fails: a named `package.json` above the file, any `node_modules` path, or a directory a `node_modules` entry links to (manifest or not), excluding a link back to the app or above it. The planner only edits files already in the snapshot, so a not-yet-existing path cannot create a package file.
- Scrubs, proposes, and accepts run the same transform. Held proposals keep the original ops and plan again on accept. `restore` is set only by the server's revert/import paths; the wire deletes a client's `restore` on every method.
- A client `hello` is forced to `paths: false`, so `path` and `watch` stay on the local refresh. The `changed` event lists `{package, file, digest}` only. `watch.links` stays inside the app; roots are package directories and existing `node_modules` `package.json` parents, not a `"tamber"` path that leaves the package.
- Reads fold the package text the walk already keys by digest. The watcher only pushes `changed`.
