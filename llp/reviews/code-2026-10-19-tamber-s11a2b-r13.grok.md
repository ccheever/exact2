# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 10 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `22b52ea8ac`.
- **Method:** one brief (sha256 `0c31189ebef2c251134763f0b7fcdc954ddbc03456a2a30c18f231b121d1941d`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING** — A design linked from a nested `node_modules` is writable — `contract/cli/src/tamber_packages.rs:161`

`in_package` only inspects `node_modules` on the app directory and its ancestors. A link such as `src/node_modules/@acme/reading` → `vendor/reading` (no `package.json`) is never considered. The canonical path `vendor/reading/reading.tamber` has no `node_modules` component and no named `package.json` above it, so the check returns nothing. `app.json` `"tamber"` may name that file: it is a plain file inside the app, the server starts, and `read_only` allows the op. `set-token` then renames onto it (`contract/cli/src/tamber_serve.rs:1435`). The same check is what `undo` uses, so the inverse is allowed too. A real directory under that nested `node_modules` is still caught, because the canonical path contains `node_modules`.

What holds: the override transform keeps a package token's written kind, drops `when`s until the app sets the token, and tracks names across the batch. Scrub uses the same transform. `restore` is server-owned (the wire strips a client's), and a local import hold is the override line. `undo` runs `read_only` before it applies. A client's `hello` is forced to `paths: false`. Direct and hoisted manifest-less links, and a file that becomes a package's, stay refused.
