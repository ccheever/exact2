# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 12 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e9cd49f87c`.
- **Method:** one brief (sha256 `7931edd662265551fd82f2f46a55533da6a3dbcc71c82faa87cff2dfe6a6659b`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

Two **BLOCKING** paths remain:

- **BLOCKING — [contract/cli/src/tamber_packages.rs:191](contract/cli/src/tamber_packages.rs:191): symlinked install directories hide package ownership.** Example: `app/node_modules → /shared/deps`, with `/shared/deps/@acme/reading → app/vendor/reading`, no package manifest, and `app.json.tamber` naming `vendor/reading/reading.tamber`. The scanner skips the first symlink because `modules` is false. With no loaded package record, the remaining checks pass and `set-token` writes the package’s file. Likewise, it misses nested dependency links inside an external linked package. Traverse relevant linked directories with cycle/budget protection, or refuse when ownership cannot be established.

- **BLOCKING — [contract/cli/src/tamber_packages.rs:172](contract/cli/src/tamber_packages.rs:172): filesystem inspection errors still fail open.** Both `flatten()` calls discard directory/entry errors; line 188 also ignores `file_type()` failures. An execute-only `node_modules/@acme` can contain a usable link to writable `app/vendor/reading` without a manifest. Enumeration fails, ownership goes undetected, and the same app-design write succeeds. Inspection failures other than expected absence need to propagate as refusals.

What checks out: the physical nested-directory regression is covered, exceeding the directory limit now refuses, and design transactions/proposals, scrub releases, and undo reach the guard.

Static review only; tests were not run because they create and modify fixtures. Existing tests do not cover these two cases.
