# Code review: web press paint keeps authored transitions, final narrow re-check (capped) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b9c8345cfb`.
- **Method:** one brief (sha256 `4ed4596e5802497bc6a902be8355b17a7e937b2b25de74be6c47f55203278ab3`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/input-glue.js:392](host/web/input-glue.js:392):** Reusing a kept declaration overwrites `keptBase` with the currently painted value. With black base text and red pressed text, releasing records red as the base. If the author changes the base to red partway through release, Firefox’s resulting transition matches that incorrect value and [line 354](host/web/input-glue.js:354) cancels it. Preserve the original base alongside each retained declaration. The new test’s endpoint checks miss this sequence.

The bookkeeping failure was reproduced in memory; the browser test could not start because `playwright-core` is missing.

Verdict: **NOT READY** — clip/origin and axis fixes look correct, but cancellation can still stop an authored transition.
