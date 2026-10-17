# Code review: a click's token on selected and outline (LLP 1122 T2b), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `5ef446fa5`.
- **Method:** one brief (sha256 `be9785f1c1d56ebf27106d179828f0b080ce36727f8a89d490918375e5c91f57`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken. The page no longer keeps a capped set of tokens. A token is its inspect session (a random prefix, new each time inspect mode starts) and a counter, and an outline naming any click of the session stays put, however many reports are on their way. Leaving inspect mode ends the session.

---

**Medium — [host/web/dev.js:492](/tmp/rv-dt-click-r2/host/web/dev.js:492): the 32-token cap can evict clicks whose reports are still queued.** Reports are serialized without a queue-length bound. Delay a POST, then make 33 selections before their outlines arrive: the first token is deleted even though its report can still be accepted. Its eventual outline fails the membership check at [line 643](/tmp/rv-dt-click-r2/host/web/dev.js:643) and scrolls the first mounted copy—the jump this change should prevent. Failed selections also consume entries and can evict the current selection’s token before a refresh. **Fix:** retain recognition independently of recent-click count, for example with a unique inspect-session prefix and monotonic counter. Add a regression that delays replies across more than 32 clicks.

The other round-one fixes hold by inspection: duplicate re-asks retain suppression; ordinary two-click and generation-refresh cases preserve their tokens; refused selections cannot suppress an untagged tool Outline. Leaving inspect clears tokens, and a full reload starts fresh. Neither queue coalescing nor overflow handling replaces outline frames; the native relay preserves `click`, while native inspection remains unsupported. Validation bounds supplied tokens, and the constant, documentation, and assertions now consistently identify the addition as tamber/1.7.

Tests were not run under the read-only restriction; no files were changed.

Verdict: **Request changes — token eviction still allows a pending click’s own outline to scroll.**