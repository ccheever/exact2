# Code review: @exact/cards ProfileHeader (stage 13 C piece 2) and the gallery's typed call arguments, round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e4a06e5ce3`.
- **Method:** one brief (sha256 `f30f7289f3edb2342e7337fc394520447a7afe9b0f448684a415a5efc629fd00`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all three taken in ef390e5cc7 (role=button on the counts; the fade as height plus a token padding; SLOTTED tied to the IR)

VERDICT: **WRONG**

1. **should-fix** — [packages/cards/profile-header.contract:172](/tmp/iosfix-rv-ph2/packages/cards/profile-header.contract:172), also line 176. The followers and following actions lack `role="button"`. Neither `press` nor `aria-label` gives a `row` button semantics: Apple’s `actsAsButton` requires a button node or an explicit role. These counts therefore appear as ordinary content rather than actionable buttons to assistive technology. Add the role to both rows; this preserves their authored layout.

2. **nit** — [packages/cards/profile-header.contract:80](/tmp/iosfix-rv-ph2/packages/cards/profile-header.contract:80). The fade decline overlooks a layout solution: `height="env(safe-area-inset-top)"` plus `padding-bottom="cards.banner-shade-reach"`, with that token set to `20px`. Under the existing content-box default, this gives the same outer height and gradient area without token names inside `calc()` or `env()` inside Tamber. Qualify this alternative against the copy before retaining the literal.

3. **nit** — [scripts/stage12-gate.mjs:519](/tmp/iosfix-rv-ph2/scripts/stage12-gate.mjs:519). `SLOTTED` now selects the correct components, but no test connects that list to the IR’s slotted components. The existing test checks that markers exist, not that `markerIds()` selects them. Assert that relationship so dropping ProfileHeader from the rendered check cannot silently recur.

The inert-action fixes and their fixtures, badge parameters, geometry expressions, and banner alias look correct. Compiler paths remain unchanged. The fixed image-scrim decline is reasonable; I found no further D0–D5 issue.

Static review only; no files changed, builds or tests run, or pixel-parity claims independently verified.
