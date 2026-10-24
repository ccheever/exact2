# Code review: the design checker's bound (rt1's PRIORITY finding), round 3 of 3, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `3f56f7a7bd`.
- **Method:** one brief (sha256 `060b0dc4e5a3566075446db2af58390cf3a1556fc10bc5028ad9bbb7bca68026`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at 3f56f7a7bd, on exact47's main; with the security pass. Two closing lines pointing at the reviewer's own scratch files are left out.

Current branch vs origin/main. 29 files, 1569 insertions, 133 deletions. One blocker, one should, one nit.

## BLOCKER

`tamber/src/edit.rs:304` — an `add-token` kind past 4096 bytes is refused in different words from the same kind in a file. `valid`'s `long` closure always calls `over_bytes("a name", …)`, and the kind goes through `ident` (`edit.rs:371`). `bound::preflight` calls `over_bytes("a kind", k)` (`bound.rs:54`). Both are `tamber-value-size` and both name the length, so the value is not echoed, but the wire says "a name is at most 4096 bytes; this one is N" and `check` / the build says "a kind is at most 4096 bytes; this one is N". `plan` returns `valid`'s diagnostic before the line is written. Metadata was aligned and locked; the kind test only checks the disk id. Measure the kind with `over_bytes("a kind", k)` and assert the two messages are equal.

## SHOULD

`QUEUE.md:8` — the dial-influence sentence describes dials × tokens and "tens of minutes" for about 13,000 of each in 1 MiB. The published timings match a one-tuple design. `influence` refolds every non-refused token at every tuple (`check.rs:1149`), and `axes` caps that product at 64 (`check.rs:501`). Six binary axes still leave room for those 13,000 dials and tokens, about 64× the stated time. The params-laying sentence matches `params.rs` `laid` (cubic), and the batch sentence matches `export_batch.rs` `take` (one blamed edit per check is edits × check). Those three costs are not blockers. State the influence cost as dials × tokens × tuples, tuples at most 64.

## NIT

`tamber/src/import/mod.rs:584` — a custom property whose name is past 256 bytes and whose value is within 4 KiB but nested past 32 is reported from the static `DEEP_VALUE` string. `source_read` then says "a value nests at most 32 functions deep" (`import/mod.rs:846`) and drops "this one nests N". `over_limit` had already built that sentence. A short name gets the full sentence through `source_bound`. The 4 KiB size wording without "this one is N" is the accepted exception. The build's importer and the server's importer share this path.

## Re-checked, not reopened

The round-2 fixes hold: one `deeper()` counter for tokens and folded functions, with the guard dropped on every return; icon aliases followed once each; selector search lazy and capped at 4096; batch removal looped, with only halving recursive; `profiles()` leaves an out-of-bounds profile colour as written; `Diagnostic::new` caps every message at 1024 bytes; reimport bounds keys and strings on the incoming document and an explicit baseline; metadata is `k=v` of the decoded value on the wire and on disk; a long CSS name is checked after the value. `Deeper` cannot stay raised across a return, and a panic aborts the release, logic-dev, and gpu-dev builds. A refused wire operation is not committed; the journaled rename runs only after `plan` and `write_checks`. In-bounds `color(--…)` profile colours are still substituted, so this range does not change their folded output. The corpus fixtures and the new limit tests were not executed.
