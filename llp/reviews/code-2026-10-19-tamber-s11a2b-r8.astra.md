# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 5 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `90680dfd46`.
- **Method:** one brief (sha256 `d3f4d1df1b6e75fb4c85540bc7210610ea72372b08b0eaa29250ae5867ce9b2d`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

Two **BLOCKING** correctness gaps remain:

- **Recovery before the first poll is missed** — [scripts/tamber-package-designs.mjs:52](scripts/tamber-package-designs.mjs:52). After a directory is first listed, remove it, let the rescan close its watcher, then recreate it before its first timer sample. `there.has(dir)` is false, so that sample silently records the replacement inode. Later polls see no change; the directory stays unwatched. Seed the polling baseline when registering a directory.

- **A new dangling package link never recovers** — [contract/cli/src/tamber_packages.rs:65](contract/cli/src/tamber_packages.rs:65). Retarget a package link to a previously unseen, nonexistent external directory. The existence/canonicalization filters omit it; only its install ancestors remain watched. Creating the external target later changes none of those directories’ inodes, so polling never rescans and subsequent design edits remain invisible. Retain/poll the consulted install path even while its target is absent.

Read-only mock probes reproduced both tracker failures. The [added regression](scripts/tamber-package-designs.test.mjs:135) creates the target before linking and waits beyond the polling interval, missing both cases.

Otherwise, metadata/digests, whole-token overrides, remove/undo, proposal and scrub transforms, proposal revalidation, read-only guards, and client path/restore filtering show no further blocker.

No files modified. Full integration suites weren’t run because they write files.
