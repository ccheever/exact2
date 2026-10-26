# Code review: the agent's settle waits for a list's owed scroll events (conformance's startend boot race), round 4, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `7b3cb2e06f`.
- **Method:** one brief per round (sha256 `638f81855285d940c43cce891edde2deacc845dfbc7f4c0013322cd16ba33006`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (narrow re-check of round 3)
- **Disposition:** none needed

**RIGHT.** The three round-3 fixes hold at `7b3cb2e06f`, and the surrounding settle behavior is unchanged.

- A JavaScript settle that uses all 16 rounds without a quiet round, and sees `scrollsFired` move during that last round (the owed-scroll wait or the round’s frame), returns `settled: false`, `reason: 'scroll-event'`. A still-owed scroll does the same. Exhausting with no delivery in that round still falls through to `settled: !inflight`.
- Conformance’s settle takes the step (`boot`, the script line, or ``tap ${id}``), labels the pages `reference` and `other`, and fails that step.
- The `scroll-event` diagnostic says the settle ran out of rounds with the reader’s scroll event still owed or still running.
