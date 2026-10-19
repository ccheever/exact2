# Code review: LLP 1121 stage 9 G2, the reader's track taken back, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the reclaim commit and round 1.
- **Method:** one brief (sha256 `c1a0b671f2aebc694a0cb67f2242571effcb335f2295fab830962fd27e61bf7a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (the extra tick masked the reclaim frame's sample).
- **Disposition:** fixed in round 2's commit.

---

Nothing is blocking. I found one remaining test gap, and no production-code defect.

1. **Should-fix — [host/linux/src/lower_tests.rs:233](/tmp/rv-g2-2/host/linux/src/lower_tests.rs:233): the extra tick masks a broken reclaim-frame sample.** If `reclaim_played` stopped setting the presented value or marking it dirty, the commit could leave opacity at the reader’s target, `0.2`. This subsequent `tick(250.)` would sample the restored running track and publish `0.8`, letting every assertion pass. **Fix:** assert ownership, track removal, epoch change, and `0.8` immediately after `commit`, before another tick. Keep the 500 ms and 1 s assertions; those correctly check continuation.

The retirement ordering, subsequent retargets, token scrub updates, held looks, and colour-track removal look consistent. Apple’s claim also holds for this change: opacity/colour transitions stay sampled, and keyframe eligibility is reconsidered on touched-node commits. The new scan adds linear work over played tracks, with bounded look checks and no allocations in the scan; I found no demonstrated performance defect.

Source review only: tests and benchmarks were not run because this checkout has no build artifacts and the review is read-only.
