# Code review: the design tool's colour param swatches and chain words, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `7b3cf1768c` (shas before the rebase onto main). The author (Claude) is not a reviewer.
- **Verdict:** two BLOCKERs (a residual's swatch, a set value's sides) and two SHOULD-FIXes, all taken in the round 1 fix commit.

1. **BLOCKER — [components.ts:185](/tmp/rv-dt-swatch/apps/design-tool/components.ts:185):** Residual tokens use `.css`, while the grid paints their per-side fallback (`srgb`). This affects the original `cards.surface = -exact-background` case: both swatches receive the same role expression and follow the tool’s scheme. Use the grid’s paint selection, retaining the role and fallback explanation as text.

2. **BLOCKER — [components.ts:185](/tmp/rv-dt-swatch/apps/design-tool/components.ts:185):** A set param’s `values[tuple]` can contain `light-dark(...)`; one CSS string does **not** mean identical sides. Copying it unchanged makes Light and Dark swatches paint alike under the tool’s scheme. Select the appropriate side using `c.dark`—the existing `dials.sides()` handles this.

3. **SHOULD-FIX — [components.ts:189](/tmp/rv-dt-swatch/apps/design-tool/components.ts:189):** A direct role default without an app override, such as `-exact-secondary-label`, gets an empty `token` from `tokenOfIn()`. Consequently its words are empty instead of identifying “the platform’s role.” Recognize direct role defaults independently of declared tokens.

4. **SHOULD-FIX — [components.ts:188](/tmp/rv-dt-swatch/apps/design-tool/components.ts:188):** Role detection misses CSS system roles such as `Canvas` and `AccentColor`, including tokens resolving to them. Those show ordinary value words instead of “the platform’s role.”

Items 1–2 block. Column order, tuple indexing, colour-only gating, and the empty-`follows` fallback look correct. No changes, builds, or tests performed.
