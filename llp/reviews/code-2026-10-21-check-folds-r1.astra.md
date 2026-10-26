# Code review: check's text overlap under a bar laid over a list (main red, agent-check's folds page), quick review, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `452d49bb2d`.
- **Method:** one brief (sha256 `f69558bbe857b7200e30139100ae042bed50fc3adba668291cba0d26238874d9`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 SHOULD-FIX: the skip rested on the middle only; a label past its bar's edge would hide a visible collision
- **Disposition:** taken in 25b6f5701e

**SHOULD-FIX — [scripts/agent-check.mjs:170](/tmp/iosfix-rv-cf1/scripts/agent-check.mjs:170):** The guard skips the entire pair using midpoint-only evidence. A bar spanning x=80–140 can have label ink spanning x=60–160: matching `beneath`/`on`/`solid` suppresses overlaps even where glyphs extend outside the opaque fill and collide visibly. Confirm the intersecting ink is covered before skipping, and add this regression.

The added unit test and probes for ordinary “GoingMaybe,” transparency, missing iOS fields, and `own`/`page` passed. The geometry issue was verified with mocked host observations; FOLDS was not run because it writes build artifacts. No files changed.
