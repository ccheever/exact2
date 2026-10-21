# Code review: LLP 1122 T1 commit 1 (Tamber server: history, revert, import, the tool's files), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `7e62930b3` (the range `9479c6464..7e62930b3`).
- **Method:** one brief (sha256 `9b06e667e8ff7c8224874f102d4de02191523a3bafabbbb8f58db9d9dc5bbb65`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 4 medium).
- **Disposition:** taken in round 2: a later reorder of an axis value the commit added is refused; a held revert keeps the digests of files it was planned on but does not edit, checked at accept; DTCG file names cut only at a character boundary (the core no longer panics); a partial revert accept keeps its selection when planned again; a nested design keeps its configured path when gone. Declined: an aggregate revert over the 1 MiB line is refused by that limit, the protocol's own for every write (LLP 1121 stage 3); a single file over it is now a named conflict.

---

Round-1 fixes address token metadata/kind checks, duplicate declarations, symlink containment, DTCG bundles, quoted filenames, and the skip cap. Per-file comparisons fix history batching. The original axis-order and stale-revert examples are addressed, but these problems remain:

1. **High — Reverting an axis addition can discard a later default change.** [tamber/src/history.rs:322](/tmp/rv-dt-1r2/tamber/src/history.rs:322)  
   The order check excludes values the inverse will remove. For `black | dim` → `black | dim | navy`, followed by a later edit to `navy | black | dim`, reverting silently removes `navy` and changes the default back to `black`. The later reorder is lost without a conflict. **Fix:** check subsequent ordering changes involving removed values too, and refuse when removal would discard that change. Add this case beside the existing reorder tests.

2. **High — “Already undone” files lose their stale-revert guards.** [scripts/tamber-server.mjs:448](/tmp/rv-dt-1r2/scripts/tamber-server.mjs:448)  
   `hold` retains digests only for files appearing in operations. Suppose a commit changed A and B, but B is already restored when the revert is proposed. If B subsequently changes, acceptance checks only A and succeeds without replanning B. The core also checks supplied digests only for operation targets. **Fix:** retain and enforce every file precondition used to plan the revert, including files producing no operations and files subsequently removed. The wire probe confirmed B was absent from the transaction’s guards.

3. **Medium — An import filename can terminate the session core.** [contract/cli/src/tamber_import.rs:145](/tmp/rv-dt-1r2/contract/cli/src/tamber_import.rs:145)  
   The new endpoint passes arbitrary names into the DTCG importer. An import with `kind: "dtcg"`, `name: "日本語"`, and `source: "{}"` reaches [dtcg.rs:386](/tmp/rv-dt-1r2/tamber/src/import/dtcg.rs:386), which slices at byte `len - 5` inside a UTF-8 character and panics. This kills the long-lived core; subsequent requests fail until restart. **Fix:** make suffix handling UTF-8 safe in both `stem` and `set_id`, and test that malformed or unusual names leave the session usable.

4. **Medium — Replanning restores items the user rejected.** [scripts/tamber-server.mjs:480](/tmp/rv-dt-1r2/scripts/tamber-server.mjs:480)  
   A partial acceptance retains the commit SHA but loses the selected edit identities. Select only `ink` from an `ink`/`edge` revert, then change an unrelated token before accepting the replacement proposal: replanning proposes both `ink` and `edge` again. Changes to the rejected item can also block the selected revert. **Fix:** persist the selection and restrict replanning and conflict checks to it. The wire probe confirmed `[ink]` became `[ink, edge]`.

5. **Medium — Large reverts still exceed the stdio limit after batching.** [scripts/tamber-server.mjs:547](/tmp/rv-dt-1r2/scripts/tamber-server.mjs:547)  
   Per-file plans are concatenated and sent through one `propose` request. Twelve files containing 1,000 changed colour tokens each produce individual planning requests of roughly 36 KB, yet the combined proposal returns 413. Thus round 1’s revert-size problem remains. **Fix:** retain the aggregate plan in the core behind an identifier, or transfer it in bounded chunks while preserving one atomic proposal and its preconditions. Add an aggregate-size regression test.

6. **Medium — Removing a configured nested design hides its history.** [scripts/tamber-server.mjs:521](/tmp/rv-dt-1r2/scripts/tamber-server.mjs:521)  
   `designPath()` returns null once the file disappears, and the fallback uses only its basename. With `"tamber": "styles/theme.tamber"`, deleting that file while the server runs changes the history pathspec to root-level `theme.tamber`. Its history disappears, and reverting its commits can report “changed no design file.” **Fix:** preserve the configured relative path independently of whether the file currently exists.

Validation used source inspection and read-only, in-memory wire probes with a stand-in core. Cargo suites were not run. No files changed.

Verdict: **Request changes.**
