# Code review: press test waits for the paint's end, quick review (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `aa76c4ff38`.
- **Method:** one brief (sha256 `d08de3420ee4852d3d03e0c373dfb35f729f2b3c92e21a30e743ebd6887c33f7`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

No BLOCKER / MAJOR / MINOR / NIT findings.

The wait at [host/web/tests/press.test.mjs:738](host/web/tests/press.test.mjs:738) is byte-identical to line 718. It only observes cleanup: a stuck marker returns `true` and fails; the unchanged final appearance assertion catches incorrect restoration.

The two-second bound is checked on frames, so it can overrun. If frames stop, the [15-second CDP timeout](scripts/agent-launch.mjs:1103) rejects rather than hanging indefinitely. Checked the polling logic with simulated frames; browser suite not run.

Verdict: Correct and equivalent; preserves detection of stuck cleanup and incorrect final appearance.
