# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 4 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `a2735374e6`.
- **Method:** one brief (sha256 `e06de49a8eafd05dc47968fd46f8e6c60571adf2a8aab5bdf8355b8bc8635469`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING** — [scripts/tamber-package-designs.mjs:27](scripts/tamber-package-designs.mjs:27), [contract/cli/src/tamber_packages.rs:94](contract/cli/src/tamber_packages.rs:94): a linked package outside the app loses monitoring if its **target directory disappears**.

Reproduction: `node_modules/@acme/notes → /work/notes`; remove `/work/notes`, allow a rescan, then recreate it without changing the symlink. The tracker closes the missing root’s watcher, while `watch.links` excludes its external parent. Recreation and subsequent design edits therefore never trigger `changed` until another watched event occurs. Retaining the path in `listed` cannot initiate a rescan. The latest regression test keeps the target directory present, so misses this case. A bounded recovery trigger and regression test are needed.

Otherwise, the metadata/digests, whole-token override transform, transaction/proposal/scrub routing, undo, read-only refusals, client `paths`/`restore` stripping, and announcement ordering check out.

Verified watcher closure and absent recovery callbacks by executing the repository tracker with an in-memory filesystem double. Full integration suites were not run because they write files. No files modified.
