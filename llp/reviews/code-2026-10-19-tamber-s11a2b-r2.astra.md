# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), round 2 of 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `0d8dd32eca`.
- **Method:** one brief (sha256 `600c29979b03ed53e76422f725de0be8120735048b96c5a2607d12d1300ac771`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

Found two blockers and one should-fix:

- **BLOCKING — Package refresh still misses supported layouts.** [tamber-package-designs.mjs:23](scripts/tamber-package-designs.mjs:23) watches only the design’s directory and parent. For `reading/dist/reading.tamber`, it misses replacement of `reading`; linked-package retargeting is also missed because installation paths are filtered out by [tamber_packages.rs:47](contract/cli/src/tamber_packages.rs:47). `package.json` changes are ignored, and an initially missing explicitly named design without `.tamber` is excluded. Clients can remain stale indefinitely. Watch the resolution inputs and relevant ancestors, including missing named files.

- **BLOCKING — The read-only guard misses app-relative package paths.** [tamber_packages.rs:187](contract/cli/src/tamber_packages.rs:187) compares only package identity and absolute path. If `app.json.tamber` names `node_modules/@acme/reading/reading.tamber`, startup accepts it as `self.design`; its app-relative name matches neither guard. An ordinary `set-token` can then overwrite the package file. Compare resolved file identities, or reject a package file as the writable app design.

- **SHOULD-FIX — Clients can enable the restoration exception.** [tamber_serve.rs:1112](contract/cli/src/tamber_serve.rs:1112) trusts public `restore:true`; the wire forwards it. A held proposal can also supply `review.importer`, which [tamber-server.mjs:925](scripts/tamber-server.mjs:925) treats as authorization. Both bypass the promised `add-token` refusal. Keep restoration authorization in server-owned state.

What checks out: package provenance, stable identities, text digests, explain-only `package_value`, and whole-token overrides. Existing app tokens retain normal set behavior; params, icons and roles do not acquire the package transform. The nine base names match the schema. Transactions, proposals and scrubs share the transform; removal restores fallback, and undo uses byte-restoring inverses. Held accepts replan against current packages.

For detected changes, baseline suppression, additions/deletions, event ordering, parameter refresh and scrub cancellation look correct. Paired read-scope hello strips paths/watch; the pre-existing absolute `app` field remains. Ordinary out-of-app package paths cannot become write targets.

Read-only Bun probes confirmed the watcher gaps, restoration forwarding, remote path stripping and scrub/refresh behavior. File-writing integration tests were not rerun. Coverage still lacks override→when→remove→undo and nonempty `package_value.whens` assertions. No-package **read replies** retain their shape; base-name writes intentionally change. The protocol bump remains the stated landing dependency.
