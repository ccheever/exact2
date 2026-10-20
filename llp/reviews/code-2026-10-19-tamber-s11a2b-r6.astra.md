# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 3 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `da4815454f`.
- **Method:** one brief (sha256 `c906b79279d47a930c926626c433391bc766b58e0b7bd491a88032821cdd3818`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — [tamber_packages.rs:63](contract/cli/src/tamber_packages.rs:63): recovery still fails for a new package target.** Retarget a package symlink to a previously unseen directory while its `package.json` is absent, then let the rescan complete. `p.is_file()` excludes that target from `roots`; `links` watches only its install parent. The [tracker](scripts/tamber-package-designs.mjs:37) retains the old canonical target and nonrecursive parent watches. Restoring the new target’s manifest and editing its design therefore triggers no rescan or `changed` event until an unrelated watched change.

Watch existing candidate package directories even when their manifests are missing. The [new regression test](scripts/tamber-package-designs.test.mjs:123) covers only a previously watched directory.

Otherwise, the metadata/digests, override transform across transactions/proposals/scrubs, whole-token semantics, undo/remove, read-only checks, client path stripping, and announcement ordering check out. No other blocking findings.

In-memory tracker checks passed for ordinary recovery and reproduced this gap with mocked filesystem events. Integration suites were not run because they write files. The working tree remains unchanged.
