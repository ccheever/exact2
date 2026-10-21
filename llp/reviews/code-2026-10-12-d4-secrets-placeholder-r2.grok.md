# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 2 (6f0ed913d), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4efc45db7addda28dab63dd559ae435d945070025c0d1fc3c3bb12778d8c443d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Astra's MINOR is taken: a placeholder never keeps an equal compiled object; test.

---

No findings.

The snapshot fix holds. `opened_with_secrets` is set once in `Store::from_names`, and every store is built there: fresh boot, reload carry, checkpoint boot, and module mirrors. Transaction checkpoint and restore put values back and leave the flag alone, so a write or a rolled-back write before readiness cannot change the choice. A reload or a checkpoint boot is a new boot and takes its own snapshot, which is the right reading of a boot whose store opened holding a granted secret.

An `else source()` row does not leave a flip. Settlement will not decide the reader until that row has settled in the pass, and settling it stores a value, so `secrets_unseen` sees the row as present and keeps the placeholder. A later pass, or a store write that only bumps the revision, decides again from the same snapshot and takes the same arm. Pass-local pending and the published flag stay together.

The diff is this rule, its journal line, the six tests, the LLP paragraph, and the web deferral. The journal text matches the LLP. The six `secrets::` tests passed.

READY
