# Code review: the dev loop's -dev bake driver rebuild, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `74680b6845`.
- **Method:** one brief (sha256 `e8303bc151fbf897760d21892f632eb17ee8e282cc0c6d385cf2270a4fc12ae8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/web/dev-bin-run.test.mjs:35](host/web/dev-bin-run.test.mjs:35) — A killed test leaves the lock directory behind. Subsequent runs retry `mkdir` for 25 minutes, then fail before reaching the marked-deck cleanup. This persists until manual removal. Use an owner-aware lock with stale-owner recovery, such as the existing `claimBuildOutput` mechanism, and retry only contention errors.

**Non-blocking**

1. [scripts/app.mjs:1252](scripts/app.mjs:1252) and [host/web/dev.mjs:535](host/web/dev.mjs:535) — Both cross the enforced 1,500-line cap: 1,516 and 1,505 under `caps.mjs`’s counting method. Shorten the added prose or consolidate existing code before running that gate.

Reviewed both diffs statically; no files edited, network used, tests or builds run.
