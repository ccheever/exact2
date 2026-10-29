# Code review: a re-held import keeps its role overrides (third pass #6), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `55f60f8531`.
- **Method:** one brief (sha256 `cfdb7ec7700080345086fe02578d13f7e53f503a589499e345813c5d6652eb4c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [scripts/design-tool-fields.test.mjs:139](scripts/design-tool-fields.test.mjs:139) — The second import races the held-list update. `clock data` does not await the established long poll, and `clock +1` advances virtual time. The import can therefore finish while proposal `b` remains absent, making the following `editOne(b)` fail despite correct code. An offline delayed-delivery probe confirmed this ordering. Wait until `changes.held` contains `b` before continuing.

**Non-blocking**

none.

No choice-preservation defect found: 96 in-memory transition checks passed, plus re-import shape checks. Full drives were not rerun under the read-only/no-network restrictions.
