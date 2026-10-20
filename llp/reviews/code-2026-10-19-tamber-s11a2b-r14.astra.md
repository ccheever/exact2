# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 11 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `1fc2382b64`.
- **Method:** one brief (sha256 `6d43f30fe180f319bc1b765ac0c4d98a89c64b7d798cd889a876f3e4bef3507c`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

Two **BLOCKING** gaps remain:

- **Nested dependencies escape the scan** — [tamber_packages.rs:177](contract/cli/src/tamber_packages.rs:177). Finding `node_modules` records it without descending. A link such as `node_modules/outer/node_modules/@acme/reading → app/vendor/reading` is therefore missed. Without the target’s manifest, with `app.json.tamber` naming `vendor/reading/reading.tamber`, `set-token` passes the guard and writes that package file. The new test covers `src/node_modules`, not this layout.

- **The scan limits fail open** — [tamber_packages.rs:171](contract/cli/src/tamber_packages.rs:171). Exceeding depth 24 or 20,000 directories stops discovery but permits a “not a package” result. A manifestless package link below that boundary produces the same writable-package bypass. Keep bounded discovery, but refuse writes when ownership cannot be established.

Other reviewed paths check out: rebased package fields, whole-token overrides retaining written kind, proposal/scrub transformation, undo’s guard, server-owned restore handling, and client hello’s removal of package paths/watch information.

Read-only checks passed for tracker baseline/change/addition/deletion behavior, announcement path stripping, and changed JavaScript syntax. Rust/integration tests were inspected but not run because they create files. No files modified.
