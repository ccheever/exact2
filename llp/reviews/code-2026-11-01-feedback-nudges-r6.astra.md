# Code review: feedback nudges and one-line notes, round 6 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `ef7c888a18`.
- **Method:** one brief (sha256 `8662dfab98f32c9dda002366af7d37d843e3091fe100d26b91c8fda002181108`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** superseded. The lead ruled (2026-11-02) that a note attaches no command output, so the stderr relay these findings concern was removed; the simplification has its own review.

1. **MAJOR — [scripts/app-cli.mjs:244](/Users/admin/x2scratch/fb1-rv-astra/scripts/app-cli.mjs:244): The 300 ms timeout discards output during backpressure.** The timer starts after exit, but still destroys stderr when it is paused for a slow downstream reader. Waiting for the parent’s subsequent `drain` cannot recover unread child bytes. Filesystem-free reproduction using HEAD’s `spawned` function: have the child perform 128 synchronous 1,024-byte stderr writes and exit 37; consume the launcher’s stderr with Python `os.read(fd, 8192)`, sleeping 50 ms between reads. The child confirmed **131,072 bytes written**, but the caller received **73,728 bytes**, with exit 37 preserved. **Fix:** suspend the inherited-pipe timeout during downstream backpressure and drain buffered output before closing; apply the grace period to an idle pipe retained by descendants.

2. **MINOR — [scripts/feedback-note.test.mjs:242](/Users/admin/x2scratch/fb1-rv-astra/scripts/feedback-note.test.mjs:242): The slow-reader regression test disables both relay paths.** Its child environment sets `EXACT_FEEDBACK=0`; `heard()` therefore returns false, so neither the stderr pipe relay nor merged-file pump executes. Both iterations exercise inherited descriptors instead. **Fix:** enable feedback with the fixture’s isolated configuration, suppress nudges through project consent or account for them in assertions, and throttle reads throughout transmission—including after child exit. Assert that each intended relay path actually executes.

The permitted test command produced 2 passes; 12 tests stopped at fixture creation because the read-only sandbox denied `mkdtemp` (`EPERM`).

VERDICT: CHANGES
