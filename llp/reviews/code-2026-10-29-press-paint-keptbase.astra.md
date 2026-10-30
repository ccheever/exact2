# Code review: web press paint keeps authored transitions, keptBase narrow re-check (once, outside the cap) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `1e5ec33b39`.
- **Method:** one brief (sha256 `aef4225ab2a81e8f397fae1000de964bf9b63d1a824cc1252c33063296b4de14`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

No BLOCKER / MAJOR / MINOR / NIT findings.

- [input-glue.js:394](host/web/input-glue.js:394): first writes and rewrites capture fresh bases; keeps preserve them per property, including mixed keep/rewrite calls; dropped properties lose their bases.
- [input-glue.js:354](host/web/input-glue.js:354): cancellation compares against the previous declaration’s baseline before it is replaced.
- [input-glue.js:280](host/web/input-glue.js:280): same-state re-press during release is guarded against an old finish callback. `endPaint` removes the state; a subsequent press creates a fresh one.

The targeted browser test could not start because `playwright-core` is missing. No files changed.

Verdict: PASS on this site by code review — previous MAJOR fixed; no new defect found.
