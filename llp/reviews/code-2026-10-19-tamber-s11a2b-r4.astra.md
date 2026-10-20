# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 1 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `193ddb3b3f`.
- **Method:** one brief (sha256 `cb8a35fd52fd9cf4ce7a64e2dc236cac9329d11e2d4dacbb6db373c82f2a5072`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — A transient package-resolution failure permanently drops its watch.** [tamber_packages.rs:50](contract/cli/src/tamber_packages.rs:50) lists roots only from successfully resolved packages; [tamber-package-designs.mjs:34](scripts/tamber-package-designs.mjs:34) closes roots missing from that list.

Reproduction: remove a package’s exported `index.contract`, let the rescan finish, then restore it inside the existing package directory. Only nonrecursive install-parent watches remain, so restoration and subsequent design edits go unnoticed until an unrelated event triggers a rescan. A staged reinstall can produce the same failure. Retain a safe watch on unresolved, consulted package directories; add a recovery test with a rescan between removal and restoration. The synchronous `cpSync` reinstall test misses this case.

Otherwise, provenance/text digests, whole-token overrides, transaction/proposal/scrub transformation, remove/undo, package-file refusals, proposal revalidation, and client `paths`/`restore` filtering check out. No other blockers found.

The in-memory tracker probe confirmed the watch loss. Full fixture suites were not run because they write files. Worktree unchanged.
