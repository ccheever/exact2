# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 9 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `52e977a24e`.
- **Method:** one brief (sha256 `0876a6f7e5494125d618ed2adb1135c7a109c02e3799c031d355fb68e5812b91`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — Hoisted package links bypass read-only protection when their manifest disappears.** [tamber_packages.rs:159](contract/cli/src/tamber_packages.rs:159) scans only `<app>/node_modules`, while [package resolution searches ancestors](contract/cli/src/resolve.rs:262).

Concrete case: `/ws/node_modules/@acme/reading` links to `/ws/apps/demo/vendor/reading`, and demo’s `app.json.tamber` names `vendor/reading/reading.tamber`. Remove the package’s manifest: loading fails, `app.packages` becomes empty, and the fallback finds neither a direct install nor a named ancestor. A transaction can then overwrite this package design; undo uses the same incomplete check. Include ancestor/nested install roots in the manifest-independent protection. The regression test currently covers only a direct scoped link.

Otherwise, the metadata, typed whole-token overrides, initial-`when` refusal, batch tracking, proposal/scrub transforms, and server-owned restore handling check out. Client hello suppresses `paths/watch`; package digests invalidate folds; announcement ordering refreshes params and cancels scrubs.

Read-only JS checks passed for baseline/change/removal announcements, path stripping, mocked directory recovery, syntax, and base-name/schema parity. Rust/integration tests were inspected but not run because they write files. No files changed.
