# Code review: refused kept answers journaled (c0f481711), 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `7c9cdccf37e383cd0b59f35f059c75502eaeb5fa1eb456d24d79a01ad2d96ed2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Both findings taken in the second commit: a standing commit journals its refusals, a restore lets them go; Grok's NIT taken.

---

1. **MAJOR — Refusals after boot remain silent.** [runner/src/runner/kept.rs:231](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/kept.rs:231)  
   `journal_kept_seeds()` drains refusals only once, at `boot_inner`. If an action or timer changes an admitted seed’s identity before readiness, settlement queues its refusal and discards the seed, but nothing subsequently journals that line. The existing identity-change test exercises this transition without checking the journal. Drain committed refusals after successful pre-readiness commits.

2. **MINOR — Rolled-back refusals retain the wrong reason.** [runner/src/runner/settlement.rs:450](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/settlement.rs:450)  
   Settlement writes directly into `self.keeping.refused`, outside the tentative states and commit checkpoint. In the existing rollback fixture, changing identity from A to B and then trapping restores the admitted seed but retains the queued A→B refusal. A later successful change to C is suppressed by resource-only deduplication, leaving B’s incorrect reason. Stage or restore refusal records with the resource state; fixing the journal drain alone would expose this error.

The decode extraction preserves the previous checks. Activation and carried/checkpoint answers remain excluded by the seed/readiness guards. Both argument lists use the existing 200-character truncation and journal access path.

All 37 kept-answer tests passed using the existing test binary; they do not assert these journal lifecycle cases. No files changed.

NOT READY
