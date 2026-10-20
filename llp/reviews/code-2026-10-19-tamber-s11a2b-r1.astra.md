# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), round 1 of 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `0400fa8b66`.
- **Method:** one brief (sha256 `14bba41674dd9998b989925c2d739e63bd90646184e136e56355a81ac096a0a3`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

- **BLOCKING — Fast scrubs bypass the transform.** It runs only in [tamber_serve.rs:1129](contract/cli/src/tamber_serve.rs:1129); [tamber_scrub.rs:76](contract/cli/src/tamber_scrub.rs:76) goes directly to planning. A first `set-token` override of a package/base token therefore fails during scrubbing, although proposing or committing it succeeds. Scrubs also bypass the new `add-token` refusals.

- **BLOCKING — Package watching misses valid changes.** [tamber-package-designs.mjs:17](scripts/tamber-package-designs.mjs:17) watches only directories of already-listed designs: creating a previously absent package design is unnoticed, and package-directory replacement lacks parent watches. Separately, [tamber-server.mjs:440](scripts/tamber-server.mjs:440) ignores an explicitly named design such as `theme.design`. Clients and active scrubs remain stale until another watched event. Confirmed with in-memory server probes.

- **SHOULD-FIX — Transformation ignores earlier operations in the batch.** [tamber_packages.rs:137](contract/cli/src/tamber_packages.rs:137) tests every operation against the original text. Two initial sets become duplicate adds; an initial set followed by a conditional set is wrongly refused; remove-then-set leaves a set targeting a missing line. Track intermediate declaration state.

- **SHOULD-FIX — The promised protocol bump remains a landing dependency.** [tamber_serve.rs:75](contract/cli/src/tamber_serve.rs:75) still advertises `tamber/1.13`. The commit explicitly defers this to 10b, but HEAD does not yet satisfy the approved `1.14` contract.

- **NIT — `resolve` also receives `package_value`.** The shared helper adds it unconditionally at [tamber_serve.rs:980](contract/cli/src/tamber_serve.rs:980), although the approved shape assigns it to `explain`. Harmlessly additive, but unnecessary.

What checks out:

- Package provenance, stable identity, text SHA-256, and whole-token overrides are implemented correctly. `package_value.whens` preserves the dropped package conditions. Existing app definitions, params, icons and roles keep their existing handling; all nine base names match the schema.
- Ordinary remove restores the package/base fallback; transformed edits retain byte-restoring inverses. Proposals use the transaction transform and are re-planned on acceptance.
- Logical package filenames and exact disk paths receive explicit read-only refusals. Alternate spellings remain outside the writable snapshot. Linked packages outside the app are legitimate; design paths escaping their package are rejected.
- Tracker baseline suppression, deletion/re-addition, publication ordering and `learnParams` refresh passed in-memory probes. Package events call `scrubs.compete()`.
- Paired **read** clients can request `hello {paths:true}` and obtain absolute dependency paths through [tamber-server.mjs:713](scripts/tamber-server.mjs:713). This is metadata disclosure, not additional filesystem access; acceptable under the current scope, which already exposes absolute `hello.app`, but it is not local-only.

The added tests cover basic metadata/refusals and held-proposal invalidation, but miss the defects above and successful override → conditional set → remove/undo behavior. No files changed; write-producing Rust/integration suites were not run.
