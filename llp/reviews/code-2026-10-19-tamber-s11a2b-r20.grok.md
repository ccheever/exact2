# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 17 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `2b20d30ac1`.
- **Method:** one brief (sha256 `44948e2f607924fd1c42bb37ff3731859a0fe04cbf523b884e25afcd988dd5a1`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** no blocking.
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

no blocking.**

A design-server write still cannot land on a package file under the 21:32 definition. `transaction` (with `write=true`, which covers proposal accept, scrub release, and revert/import hold) and `undo` both call `read_only` on a fresh `params_of` load. For a real path that canonicalizes:

- (a) `in_package` reads each ancestor below the app and returns `Err` on any `package.json` error other than `NotFound`, so a named manifest is refused.
- (b) The same function refuses a real path that contains a `node_modules` segment.
- (c) `package_roots` still holds every root the load resolved, including the failure path's source-graph roots, and a failed load (unreadable Contract, `app_root` failure, or a source error) also refuses any file under a directory below the app that has a `package.json`.

An op whose file is not in the snapshot is rejected by `plan` before `commit`. Journal replay and Contract ops under `owned()` stay outside this definition.
