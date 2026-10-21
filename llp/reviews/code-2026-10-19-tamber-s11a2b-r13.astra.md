# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 10 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `22b52ea8ac`.
- **Method:** one brief (sha256 `0c31189ebef2c251134763f0b7fcdc954ddbc03456a2a30c18f231b121d1941d`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** no blocking.
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

No **BLOCKING** findings in `22b52ea8ac` at the requested security, read/write correctness, and data-loss threshold.

What checks out:

- [Package protection](contract/cli/src/tamber_packages.rs:126) covers wire names, canonical paths, scoped and hoisted links without manifests; undo repeats the check.
- [Override transformation](contract/cli/src/tamber_packages.rs:293) tracks batch names, preserves written kinds, drops package whens, and rejects an initial conditional set. Transactions, proposals, and scrubs use it; params, icons, and roles are not mistakenly transformed.
- Package provenance, text digests, explain’s `package_value`, and remove/undo behavior match the design. Package metadata remains absent for apps without package designs.
- [Client hello](scripts/tamber-server.mjs:714) forces `paths: false`. The existing app-directory disclosure remains within paired read scope; package paths and watch directories are not newly exposed.
- [Refresh handling](scripts/tamber-server.mjs:369) preserves reply ordering, refreshes params, cancels competing scrubs, and rechecks held operations on acceptance.

Reviewed the Rust and JS regressions. In-memory tracker checks passed for baseline, changed/added/deleted packages, and path-free events. Integration tests were not run because they create files. No files changed.
