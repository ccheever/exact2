# Code review: conform --linux: synthetic-decorations' rangeFrame held to the Linux host's specified 'unavailable' (theme2 04:27), round 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `984a608543`.
- **Method:** one brief (sha256 `f27a9714e7c1d299c327ca1ed3bffecf19113abfbd27ed6b8295cfc57c02825b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking, should-fix or nit
- **Disposition:** none needed

Blocking: none. Should-fix: none. Nit: none.

Reviewed all three commits. `tap measure` executes, settles, asserts `geometry === 'unavailable'`, then ends the Linux comparison. Mismatches and exceptions remain failures. Both round-1 nits are addressed, including the field-local geometry wording in QUEUE.md.

Read-only review; no files modified or builds/tests run.
