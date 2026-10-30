# Code review: the Mac's card roles draw the under-page grey (kernel table), round 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `de51e5df92`.
- **Method:** one brief (sha256 `d3448109bac8ea386c2754da6c85c8d4f34d600973cc59fcfe057c8a26301194`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed. The full Mac suite reruns on the final tip.

## Brief

Code review, ROUND 3 of 3 (kernel table change: the Mac's colour roles), of branch x2/mac-surfaces: `git log -p origin/main..HEAD -- . ':!llp/reviews'` in this worktree (6163743560 the change; 198f8b54a4 round 1's fixes; the next code commit round 2's: the sheet card test asserts the page paints no layer colour after display; the review records are docs). Read-only: no checkout, restore, stash, reset or edits. Rounds 1 and 2 are in llp/reviews/code-2026-10-30-mac-surfaces-r{1,2}.{astra,grok}.md.

Review the WHOLE change fresh with round 1's checklist (in those records' briefs), and whether each round-1 finding is resolved:
- the split card test now asserts the card's layer fill against AppKit's under-page grey resolved in the card's appearance, and that the ground paints no layer colour;
- the sheet card test checks the card's role name, an opaque card pixel, and that it stands apart from the sheet's own ground in the capture (the author found a dark sheet draws both a step lighter than a resolution made outside it, so it no longer compares the capture to that resolution): is that still a meaningful test of 'the card paints its own fill while the ground stands on the sheet's material'?
- SurfacePairsMacTests' doc comment now says what the increased-contrast legs prove;
- LLP 1095's table rows and contract-for-agents.md's surface-pairs sentence are updated: accurate?
Also: the full Mac suite was 301 classes, 0 failed on 6163743560; the kernel tests, fmt and clippy are clean.
Give findings as BLOCKER / MAJOR / MINOR / NIT with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

## Round-1 findings

Resolved. `SplitMacTests.swift:274-283` calls `display()`, asserts the ground's layer colour is nil, unwraps both sRGB conversions, and compares the card's layer fill to `NSColor.underPageBackgroundColor` resolved in the card's effective appearance. The style fallbacks (`#f2f2f7` / `#1c1c1e`) are not that grey, so a failed lookup fails the compare.

Resolved by `2291a85d02`. `VisibilityMacTests.swift:152-156` displays both views, then asserts `page.layer?.backgroundColor` is nil and the card's is not. That is the same write `BoxLayerMac.swift:89` makes: `standsOnSheetGround` forces `boxPlan.fill` to nil, and `applyBoxLayer` stores that on the layer. `display()` takes `wantsUpdateLayer` (`NodeViewMac.swift:1061-1089`) into `updateLayer` → `applyLayerPaint`, so the assertion sees the paint, not only the predicate. If the suppression stopped, the ground would hold `windowBackgroundColor` and the nil check would fail. Opaque window grey (`#1e1e1e`) and under-page grey (`#282828`) still differ by about `10/255`, which is why the red-channel check alone could not prove this.

Resolved. `SurfacePairsMacTests.swift:8-11` says the increased-contrast legs prove the `contrast: true` lookup agrees with AppKit's named high-contrast appearances, that those names resolve as plain Aqua (`SystemColor.swift:309-323`, `drawsHighContrast` is nil), and that the legs prove no contrast change.

Accurate. `llp/1095-platform-colours.rfc.md:252` and `:254` name `underPageBackgroundColor` for both card roles. The note matches the measurement already in LLP 1115 and `roles.rs:840-843`: on macOS 27 the window's and a control's backgrounds are one colour, the previous cell was `controlBackgroundColor`, and `grouped-background` stays `windowBackgroundColor` because the sheet and split grounds recognise that name. Fallback columns stay the Apple hexes, which did not move. `docs/contract-for-agents.md:4187-4188` limits "white in light mode, so it vanishes" to iOS and the web, and says both Mac card roles are under-page grey. The web neutral token for the grouped card is still `#ffffff` in light (`host/web/neutral.css`).

## The sheet test

It is still a meaningful test of "the card paints its own fill while the ground stands on the sheet's material."

The ground is a sheet window (`VisibilityMacTests.swift:138-142`) whose style is unwritten `windowBackgroundColor`, so `standsOnSheetGround` (`SheetsMac.swift:104-106`) is the predicate that skips the fill. After `display()`, that layer colour is nil, and the card's is not. The capture then requires the card pixel at `(100, 50)` to be opaque and its red to differ from the ground at `(230, 150)` by more than `0.02`. The page is `240×160`, the card is `(16, 16, 200×80)`, and the view is flipped, so those points are the card and the ground. The ground sample is the viewport's own background (`PresenterMac.swift:175-178`, `windowBackgroundColor`), which is what AppKit draws once the page paints none.

The `sys` read at `:158` only echoes the style this test wrote. Colour identity for the name is the split layer compare and `SurfacePairsMacTests`, which resolves the compiled fixture through `BatchValue.channels` and checks it against AppKit on its own. Not comparing the sheet capture to a resolution made outside the sheet is the right call: a dark sheet draws both a step lighter than that resolution.

## Fresh checklist

Only the fourth column of `secondary-background` and `secondary-grouped-background` changes (`kernel/tables/schema.json:5894`, `:5924`). `underPageBackgroundColor` matches `native_name_ok` (`kernel/src/style/roles.rs:596-607`) and is an `NSColor` class property from macOS 10.8, under the host floor of 14.0 (`host/apple/build.mjs:1410`). `grouped-background` stays `windowBackgroundColor`. Grouped-list cards keep the `exactGroupedCardColor` metric (`schema.json:6143`); fields, the frame-health card, and the Inspector do not read the card role names. The Apple host emits `r.macos` only on macOS (`host/apple/src/style.rs:611-616`). Linux maps the role names onto its own materials. `SnapshotStyle.named` resolves the new name through the same dynamic lookup (`SnapshotStyle.swift:144`). Mac `check` is unimplemented (`Agent.swift:367`); contrast judgements use the Apple hex columns, which did not move. The digest pin is `kernel/src/wire/codec.rs:732` (`0x497dcbf7076a8a19`). The kernel test pins both card names, the grouped page, and the Apple and neutral columns. `QUEUE.md:3` is the dark tertiary inversion. Touched sources stay under the 1,500-line cap (`roles.rs` 1417, `SplitMacTests.swift` 1261, `VisibilityMacTests.swift` 273).

The Mac suite figure is for `6163743560`, before the later Swift assertion edits. The new sheet lines follow the paint path above. The kernel tests, fmt, and clippy being clean matches the table, the digest pin, and the Rust test.

No BLOCKER, MAJOR, MINOR, or NIT findings.

VERDICT: CLEAN
