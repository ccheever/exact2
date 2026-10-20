# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 2 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `220397fce7`.
- **Method:** one brief (sha256 `e9a0a41474b3b834beb732e6cedd49e1d33dfe4239f678b388ae7eaf7e87e326`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — A temporarily missing `package.json` still loses the package watch.** [tamber_packages.rs:63](contract/cli/src/tamber_packages.rs:63) excludes the consulted package directory when its manifest is absent. The tracker then [closes its recursive watch](scripts/tamber-package-designs.mjs:34), leaving only non-recursive watches above it. After that rescan, restoring the manifest inside the existing directory—and subsequent design edits—cannot trigger refresh, leaving clients stale until another watched event occurs.

Watch the existing candidate package directory even while its manifest is missing. Add a regression: remove manifest → await rescan → restore manifest → edit design → assert announcement. The missing-export regression keeps the manifest present and misses this case.

Otherwise, the provenance/digest fields, whole-token overrides, batch handling, proposal/scrub transforms, undo/remove behavior, file-identity refusals, proposal revalidation, and stripping of client `paths`/`restore` check out on inspection. No additional blockers found.

Confirmed the tracker failure with in-memory filesystem stubs; changed JavaScript parses. Full suites were not run because they create files. Repository unchanged.
