# Code review: LLP 1121 stage 9 G2, the reader's track taken back, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the reclaim commit.
- **Method:** one brief (sha256 `db92b2ba01e4842571bcbf407588efbefae4049fbd058c868613778688abb58f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nothing in the code; the test reclaimed at time zero; a per-frame cost nit).
- **Disposition:** the test fixed in round 1's commit.

---

Nothing blocking found.

1. **Should-fix — [host/linux/src/lower_tests.rs:220](/tmp/rv-g2-1/host/linux/src/lower_tests.rs:220): the test reclaims at time zero.** `h.now_ms` is still `0`; `commit_effects` clamps the receipt’s `250 ms` to that clock. Reclamation therefore precedes the first advance, and `h.tick(250.)` checks ordinary progression afterward. An implementation that incorrectly restarted the original curve at reclamation would pass. **Fix:** advance to 250 ms before applying `SetLooks`, assert opacity ≈0.8 immediately after the commit, then verify continued progress and completion at the original deadline.

2. **Nit — [host/linux/src/lower.rs:179](/tmp/rv-g2-1/host/linux/src/lower.rs:179): eligibility changes incur repeated frame work.** While many reader-owned transitions coexist with an unrelated engine animation, every host presentation resolves all played nodes and checks their properties, although their look/style rows are unchanged. This adds linear work beside the existing retirement scan. **Fix:** reclaim affected tracks during the existing commit/motion-sync processing of touched nodes; keep retirement in `play_transitions`.

The production logic otherwise looks consistent: reclamation retains the original curve, start and spring velocity; delegation has already cleared pending state; holds remove played ownership; reinserting into `running` restores frame demand. The hand-over predicate prevents immediate redelegation, and the field borrows are disjoint. Keeping transforms delegated is appropriate for these opacity/colour overlays.

Android replaces surviving layers’ track lists or removes unused layers, so the cached reader track is accounted for. Apple delegates only `D` transitions; its animation eligibility is recomputed for touched nodes and withdrawn specs remove the Core Animation tracks.

Tests were not run: the checkout has no build artifacts and the review is read-only.
