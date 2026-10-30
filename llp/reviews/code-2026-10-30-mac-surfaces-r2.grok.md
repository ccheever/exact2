# Code review: the Mac's card roles draw the under-page grey (kernel table), round 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `355a41e1c7`.
- **Method:** one brief (sha256 `5d5ae6e177864bc409b37d3e6da05e4d15790e49ff6157c306f8a242cd590a7b`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** the sheet card test now asserts the page paints no layer colour after display (and the card does).

## Brief

Code review, ROUND 2 of 3 (kernel table change: the Mac's colour roles), of branch x2/mac-surfaces: `git log -p origin/main..HEAD -- . ':!llp/reviews'` in this worktree (6163743560 the change; 198f8b54a4 round 1's fixes; the review records are docs). Read-only: no checkout, restore, stash, reset or edits. Round 1's reviews are in llp/reviews/code-2026-10-30-mac-surfaces-r1.{astra,grok}.md.

Review the WHOLE change fresh with round 1's checklist (in those records' briefs), and whether each round-1 finding is resolved:
- the split card test now asserts the card's layer fill against AppKit's under-page grey resolved in the card's appearance, and that the ground paints no layer colour;
- the sheet card test checks the card's role name, an opaque card pixel, and that it stands apart from the sheet's own ground in the capture (the author found a dark sheet draws both a step lighter than a resolution made outside it, so it no longer compares the capture to that resolution): is that still a meaningful test of 'the card paints its own fill while the ground stands on the sheet's material'?
- SurfacePairsMacTests' doc comment now says what the increased-contrast legs prove;
- LLP 1095's table rows and contract-for-agents.md's surface-pairs sentence are updated: accurate?
Also: the full Mac suite was 301 classes, 0 failed on 6163743560; the kernel tests, fmt and clippy are clean.
Give findings as BLOCKER / MAJOR / MINOR / NIT with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

The split card test now proves the paint. `SplitMacTests.swift:276` asserts the ground's layer colour is nil, which is the sibling at `:254`. `:280-283` reads the card's layer fill, unwraps both sRGB conversions (so `nil == nil` cannot pass), and compares them to `NSColor.underPageBackgroundColor` resolved in the card's effective appearance. `applyStyle` already writes that layer colour through `applyClipRadius`, and `display()` goes through `updateLayer` → `applyLayerPaint`, so the assertion sees the painted fill. The fallbacks in the style (`#f2f2f7` / `#1c1c1e`) are not under-page grey, so a failed lookup fails the compare.

The sheet test is still a meaningful test of "the card paints its own fill while the ground stands on the sheet's material." `standsOnSheetGround` (`VisibilityMacTests.swift:152`) is the predicate `BoxLayerMac.swift:89` uses to skip the ground fill, so the ground is left to the sheet. The viewport is a scroll view with `drawsBackground = true` (`PresenterMac.swift:175`), and the ground sample `(230, 150)` sits outside the card `(16, 16, 200×80)` in a flipped view, so it is that background showing through. The card pixel must be opaque (`:158`), which fails if the card is left clear like the ground. The apart-check (`:159`) is red only, so the ground's transparency can no longer satisfy it. Not comparing either capture sample to a resolution made outside the sheet is the right call: the author measured that a dark sheet draws both a step lighter than that resolution, and the old `cgColor` compare was a resolution, not the picture. Colour identity for the name lives in the split layer compare and in `SurfacePairsMacTests`, which resolves the compiled fixture through `BatchValue.channels` and checks it against AppKit on its own. The `sys` read at `:155` only echoes the style this test wrote; it is not the colour proof.

`SurfacePairsMacTests.swift:8-11` now says what the increased-contrast legs prove: the `contrast: true` lookup agrees with AppKit's named high-contrast appearances. On the Mac those names resolve as plain Aqua (`SystemColor.swift:309-323`, `drawsHighContrast` is nil), and a real view with the system setting on takes that same named-appearance branch (`SystemColor.swift:80` and `:163-165`). The legs prove no contrast change. They do not claim to flip the accessibility setting itself.

The docs match the table. `contract-for-agents.md:4187-4188` limits "white in light mode, so it vanishes" to iOS and the web, and says both Mac card roles are under-page grey. That matches the schema: both `macos` cells are `underPageBackgroundColor`, and the web neutral token for the grouped card is still `#ffffff` in light (`host/web/neutral.css`). Fallback columns in LLP 1095 stay the Apple hexes, which did not move. `QUEUE.md:3` is the dark tertiary inversion the commit describes (`#1e1e1e` inside `#282828`), and it is the only queue edit on this branch. The LLP 1071 line that differs from today's `origin/main` is main moving after the merge-base (`48653eb5cb`), not this change.

Fresh pass of the round-1 checklist: only the fourth column of `secondary-background` and `secondary-grouped-background` changes (`schema.json:5894`, `:5924`). `underPageBackgroundColor` matches `native_name_ok` (`roles.rs:596-607`) and is an `NSColor` class property from 10.8, under the host's 14.0 floor. `grouped-background` stays `windowBackgroundColor`, which `SheetsMac.swift:94-105` and `SplitMac.swift:654-658` still key on. `controlBackgroundColor` remains on the tertiary rows, `FrameHealthOverlayMac`, and the field fallback. The Apple host emits `r.macos` only on macOS (`host/apple/src/style.rs:611-616`). The snapshot renderer resolves the new name through the same dynamic lookup (`SnapshotStyle.swift:144`). Mac `check` is still unimplemented (`Agent.swift:367`). The digest pin is `kernel/src/wire/codec.rs:732` (`0x497dcbf7076a8a19`); I did not recompute it. The kernel test pins both card names, the grouped page, and the Apple and neutral columns. Touched sources stay under the 1,500-line cap (`roles.rs` 1417, `SplitMacTests.swift` 1261).

The Mac suite figure is for `6163743560`, before the Swift edits in `198f8b54a4`. I did not recompile those tests. The kernel tests, fmt, and clippy being clean is consistent with the table, digest, and Rust test.

VERDICT: CLEAN
