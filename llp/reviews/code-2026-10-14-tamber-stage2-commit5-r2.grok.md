# Code review: LLP 1121 stage 2, commit 5, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `6bac852364d66dbbda478fa3c623f53b0303debda4bc5db8ad42b734d30efe81`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra fix; Grok ship.
- **Disposition:** fixed in round 2's commit

---

No findings.

The six round-1 fixes hold, and none of them breaks the fixture's existing theme path.

- `accent.soft` varies by `dark-style` (`#1a5c8a` / `#2b7bb5` on the dark half). The circle's `fill` is that token; `stroke` stays the constant `accent.fill` (`#1d9bf0`). SVG paint goes through `ColorValue::parse_light_dark`, which resolves a whole token name.
- `drop-shadow(0 0 4px accent.soft)` is valid (`0` is a bare length). The colour word is parsed the same way, the token is retained because it is a word of a colour row, and a slice change re-parses it. `drop-shadow` does not need to be a synthetic colour function for that.
- `text.label = -exact-label` is a token whose value stays the role spelling. There is no `label` override, so the row stays `ColorValue::Role` and the Mac host resolves `labelColor` with Increased Contrast.
- The swap button's `-exact-mode` ternaries are computed pins. Both arms (`dark-style:dim`, `dark-style:black`) are pins of this design. The runner reads the pin string first, retuples, and sets themed rows again. The Dim/Black labels staying put is what makes the colour move visible.
- `transition="background-color 400ms"` is an ordinary paint target on the card. Default timing is `ease`. The card is bordered, so the Mac host samples it in the motion engine.

The new `QUEUE.md` bullet belongs in `QUEUE`. The two `light-dark()` colours and the `400ms` transition are the fixture's `surface.card` pair. `#4a5058` is about the `ease` sample at 200 ms of the light half `#f1f3f5` blended toward the dim dark half `#1e2732` (around `#484f58`), and it is lighter than both dark ends (`#16181c`, `#1e2732`). A dark-to-dark blend of those ends is about `#1c242e`. The note records that the same grey showed up with literal values and with `setMode`, and it leaves the cause open. This commit does not change the kernel or the Mac presenter, so the interpolation bug stays queued.

VERDICT: ship
