# Code review: the painted popover's default placement and AppKit live margins, round 1, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `9d62a0ae9f`.
- **Method:** one brief (sha256 `870d97d5ee1374e6d639aa956ecd3cdf0147ce28d5449272ac8090c2814e923e`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in afd265f7b2

**`.platform` no-fit drops the old pane clamp.** `PositionArea.swift:124` returns `current.point` with no shift. `popUpPoint` (`MenusMac.swift:443`) passes the pane as `bounds` and `.platform`, and a fresh `History` so `choice` is the base. A menu wider than that pane used to be slid inside it (`max(min, min(point, max - size))`) before the overflow switch. Now every try fails and the point stays on the unshifted base.

Failing case: pane `(0,0,400×600)`, anchor `(380,570,20×30)`, menu `450×80`. No try is contained (450 > 400). New result `(380, 600)`. Old clamp `(0, 520)`. AppKit then fits the screen, not that pane. `.fixed` still matches the old clamp for an in-bounds anchor on the base try (both edges, start wins if the box is larger). `.absolute` uses the page and only the start edges, which is the authored-area limit.

**`fallbacks == "none"` is both “omitted” and “authored none”.** `PositionArea.swift:77` rewrites it to `flip-block, flip-inline, flip-block flip-inline` whenever `area == "none"`. Call sites pass `style["position_try_fallbacks"]?.string ?? "none"` (`MenusMac.swift:574`, `MenusIOS.swift:767`), so an authored `position-try-fallbacks: none` is the same string. CSS keeps the UA area and disables tries. `axes("none")` (`PositionArea.swift:164`) is physical bottom + span-right `(1, 2)`, which is `block-end span-inline-end` in LTR, and line 76 sets `x = -1` in RTL. An authored non-`none` list with no area keeps that base and uses the list. The keyword `none` does not.

Failing case: no `position-area`, `position-try-fallbacks: none`, anchor `(12,560,30×30)`, box `300×200`, layer `400×600`, `.fixed`. Code takes flip-block and returns `(12, 360)`. With tries disabled the base is clamped to `(12, 400)`. The same flag flips an authored `position-area="none"` (admitted in `styles_rows.rs`); that keyword used to clamp, and the updated expectation at `PositionAreaTests.swift:43` (`(280, 510)` instead of `(275, 535)`) locks the string `"none"` to the UA flips.

**The chooser never passes direction.** `popUpPoint` (`MenusMac.swift:443`) omits `rtl`. Painted layout passes `documentDirection == "rtl"` (`MenusMac.swift:576`). An RTL chooser with no area therefore spans physical right. Anchor `(300,100,40×30)`, menu `100×40`, pane `400` wide, room on both sides: inline-end is `x = 340 - 100 = 240`; `popUpPoint` returns `x = 300`.

**History, order, margins, keyboard.** Option order is base, flip-block, flip-inline, both. `flip-block` swaps top/bottom and `flip-inline` swaps left/right; the bottom-right case `(280, 510)` is that double flip with `right: 5, bottom: 5` moved to left/top. The remembered index is kept while its margin box fits, including after the viewport grows, and a direction change clears it because `rtl` is in the key. The stored fallbacks string is the input `"none"`, and the index still addresses the substituted list. iOS still passes the keyboard-shortened rect (`MenusIOS.swift:743–771`); the center clamp in the `defer` at `747` is unchanged.

**Tests.** These fail on the old clamp and match CSS: `PositionAreaTests.swift:54` `(12, 360)` (flip-block; RTL `:55` is the double flip because span-left also misses), `:43` `(280, 510)`, `PopoverMacTests.swift:341` `(10, 210)` and `:349` `(240, 190)`, and the iOS `maxY <= anchor.minY` check. `:58` `y == 0` and `:59` `(100, 230)` are right for `.fixed`, and the old clamp produced them too. Nothing covers `.platform` with area `"none"` and no try fitting, or authored `position-try-fallbacks: none`.

**QUEUE.md:3** is a glass-blur note under the theme2 20:05 stamp. The sd gap is about 20% (2.5→3.0, 4.8→5.7). It does not state the placement ruling. That same commit adds `testAPopoverFromTheBottomLeftOpensAboveItsInvoker`, which is the placement fix. The `game/render` hunks in `origin/main..HEAD` are main’s sun-cascade commit, not these two.
