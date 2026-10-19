# Code review: Tamber re-import judged whole (theme2's option B), round 3, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `7b979de1cf`.
- **Method:** one brief (sha256 `7b6ff24c80e8f0ea2f4be44bae09719811eb5bb1b1c2e7a95d9b3566ec20000b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; two P3s (rounds' doc; the test's survivor assertion)
- **Disposition:** both taken in 3207593927

Nothing blocking in either commit.

1. **Low (P3): stale documentation.** [export_batch.rs:192](/tmp/rv-optb-3/tamber/src/export_batch.rs:192) still promises “at most four rounds” and describes stopping only in terms of pins. Update it to include refusals and clearing.
2. **Low (P3): survivor assertion permits an empty result.** [export_dials.rs:499](/tmp/rv-optb-3/tamber/tests/it/export_dials.rs:499) uses `.all()`, so dropping `b` too would pass. Assert its black `SetToken` explicitly. The `["a", "c"]` assertion catches the original regression.

Termination holds: `Done` never resets `pinned`; after three pin rounds, another request refuses. `Kind` regenerates fixed operations or clears/refuses; cleared members cannot return. This bounds judging rounds per set by `4N + 1`. Both refusal paths set `moved`.

`judged_on` is equivalent here because `cur` comes from `check(current, vocab)`. Each accepted round performs one whole-proposal check; refusal isolation and non-colour canonicalization can add checks.

Static review only; no edits, builds, or tests.
