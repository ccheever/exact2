# Code review: @exact/cards ProfileHeader (stage 13 C piece 2), narrow re-check after round 2 (theme2 04:20), 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `2a34740a61`.
- **Method:** one brief (sha256 `eac9e5d2275f02fb8750c81fe62bd0dc25afdaf43109ccc9daf203c0cd88810d`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** nothing blocking; nit 2 taken in 9dfe3cdba4 (a slotted component found by its source's slot); nit 1 noted (the terminal rounds the fade's 20px padding to a 16px row, a raw geometry difference on the terminal only)

VERDICT: **RIGHT WITH NITS**

1. **nit — [packages/cards/profile-header.contract:80](/tmp/iosfix-rv-ph3/packages/cards/profile-header.contract:80).** The fade preserves outer height and gradient area on graphical hosts, including a zero safe area. “Every host” needs one qualification: the terminal rounds padding to whole 16px rows, so the new 20px padding becomes 16px; the previous height retained 20px. This is a raw geometry difference, not a blocking graphical regression.

2. **nit — [scripts/library-gate.test.mjs:32](/tmp/iosfix-rv-ph3/scripts/library-gate.test.mjs:32).** The assertion correctly protects today’s `PostCard` and `ProfileHeader`, but `parts.some(p => p.slot)` misses slots whose holder lacks `part=`—explicitly permitted by D5. An existing export could gain such a slot while remaining absent from `SLOTTED`, and this assertion would pass. Use slot-chain metadata or an explicit component-level slot flag; apply the same predicate to the marker check at line 46.

The button roles preserve press handlers and labels; parameter ordering reaches the design tool correctly; the QUEUE changes match the intended decisions. **No blockers found.**

Static review only; no files changed or tests run.
