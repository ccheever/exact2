# Code review: caps: collection/tests.rs back under 1500 (a pure move), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `5d69fb92e2dafb84a76962d9b19f2cc74cca131dd57aa331e76537098260fc3f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** ship (both).
- **Disposition:** none needed

---

I'll review `HEAD` as a pure move only: same tests and helper, same behavior, and correct module wiring. No checkouts, restores, or edits.The commit touches two files and claims a pure move. I'll compare the extracted tests and helper against the new module, then check how the module is wired.**VERDICT: ship**

`cfa56cd` is a pure move. The helper `key_reuse_dependency_plan` and the two tests are byte-identical to the block that left `runner/src/instance/collection/tests.rs` (same bytes from the function through the last test's closing brace). Nothing else in that commit changes behavior.

- Both tests are still `#[test]` with the same bodies. The parent file had 32 tests; it now has 30, and `key_reads_tests.rs` has the other 2. The helper is only called from those two tests.
- Wiring matches the existing splits (`fill`, `ownership`, `rekey`, …): `#[path = "key_reads_tests.rs"] mod key_reads;` sits inside `tests.rs`, which is already `#[cfg(test)]`. `use super::*` therefore still resolves `code`, `env`, `values`, `Plan`, `Value`, and `RegionKind` the same way those siblings do.
- The line cap counts `split('\n')`. `tests.rs` goes from 1514 to 1438, under the 1,500-line limit.
