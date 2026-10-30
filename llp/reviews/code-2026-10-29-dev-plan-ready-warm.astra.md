# Code review: dev loops say ready once listening, warm-run narrow check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d0f586fd09`.
- **Method:** one brief (sha256 `aabde177de58df31fc361e634023a974f4439d02826b24997bb7eb02d8568c70`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MINOR** — [host/web/tests/dev-plan-ready.test.mjs:89](host/web/tests/dev-plan-ready.test.mjs:89): A failed warm run—including its 600-second timeout—leaves its rejected promise in `warmed`. The second case immediately inherits that failure without attempting its scenario. Evict rejected promises or memoize only success.

- **MINOR** — [host/web/tests/dev-plan-ready.test.mjs:62](host/web/tests/dev-plan-ready.test.mjs:62): `delay = 0` omits overrides but still inherits both timing variables from `process.env`. If exported by the parent, the warm run still waits. Explicitly remove `EXACT_DEV_LISTEN_DELAY_MS` to guarantee the knob is unset.

Verdict: Two MINOR findings, none MAJOR/BLOCKER; cleanup and fixture locking are correct, and the timed cases retain their proof.
