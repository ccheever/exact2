# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 6 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `9f15631b41`.
- **Method:** one brief (sha256 `fb9ecf97db0cb584d1bb135537165c2e0f1a90023a774b70b36c0913d7cde55c`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — [tamber_packages.rs:269](contract/cli/src/tamber_packages.rs:269): linked packages can become writable when their manifest disappears.** With `node_modules/@acme/reading → <app>/vendor/reading` and `app.json.tamber` naming that package’s design, the server stores its canonical `vendor/reading/...` path. If the package’s `package.json` becomes missing or malformed, loading returns empty `app.packages`; `in_package` then finds neither a named ancestor nor a `node_modules` component. A transaction can consequently modify the package file.

Check membership against canonical installed-package roots independently of manifest/loading success, as `tamber_contract::owned` does. The existing broken-import test misses this combination of a symlink and a missing manifest.

Otherwise, the metadata/digests, whole-token override transform, batch/`when` handling, undo, proposal revalidation, remote hello filtering, and watcher recovery check out. The nine base names match the schema.

JS syntax and in-memory tracker checks passed. Integration tests were reviewed but not run because they write fixtures. No files changed.
