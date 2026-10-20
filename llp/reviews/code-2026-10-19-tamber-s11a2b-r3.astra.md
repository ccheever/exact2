# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), round 3 of 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `923481a154`.
- **Method:** one brief (sha256 `b7abf5fd1c6a08ea5e0ced3be7b218ce7385cca3114be8da667ea29e5ce3851d`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

- **BLOCKING — [tamber_packages.rs:197](contract/cli/src/tamber_packages.rs:197): Read-only protection fails open on source-load errors.** `params_of()` returns empty `packages` when loading fails. If `app.json` names a package’s design, breaking an unrelated import then issuing `set-token` bypasses this guard and permits writing that package file. Ownership must survive failed loading, or writes must refuse when ownership is unknown.

- **BLOCKING — [tamber_packages.rs:62](contract/cli/src/tamber_packages.rs:62), [tamber-package-designs.mjs:32](scripts/tamber-package-designs.mjs:32): Watchers can permanently lose a reinstall.** Remove `node_modules/@acme`, allow a rescan, then recreate it: neither its parent nor an existing ancestor is watched for recovery. The app watcher also filters out `node_modules`. Subsequent package edits produce no refresh or scrub cancellation. Watch replacement boundaries and reattach missing directories.

- **SHOULD-FIX — [tamber-server.mjs:1494](scripts/tamber-server.mjs:1494): CLI imports miss the restore fix.** `importTheme()` still submits ordinary `propose/hold`, which uses `restoring=false`. An import introducing an app override such as `reading.card` therefore hits the new `add-token` refusal. The server’s `import` route correctly preserves restoring through acceptance; the CLI needs equivalent handling.

The normal paths check out: package metadata/identities and text digests; whole-token overrides and dropped-whens explanation; batch tracking, first-set `when` refusal, remove and undo; shared transaction/proposal/scrub transformation; and the nine base names matching the schema. Params, icons and roles are not accidentally transformed. Healthy-graph file aliases are protected, and package paths escaping their package are refused.

Paired-read `hello {paths:true}` reaches Rust as `paths:false`; no package filesystem paths or `watch` escape. The pre-existing app-directory disclosure remains. Event baseline, deletion/addition, publish ordering, parameter invalidation and scrub competition look correct when notifications arrive. Package-free read payloads remain unchanged; base-name writes necessarily differ, qualifying literal requirement (d). The protocol bump remains the stated landing dependency.

Checks: JS parsing and in-memory tracker/wire checks passed and reproduced the watcher/import gaps. Integration tests were inspected, not run because they write fixtures/build artifacts; they miss these cases. No files changed.
