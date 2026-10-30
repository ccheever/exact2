# Code review: a dimmed control face's greys from Core Graphics (iOS host), round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `8470e38325`.
- **Method:** one brief (sha256 `43b580a511fec8821d69c630d06c308e9ac1e1591fb5a0c26a9517da3755572a`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** Fixed in 4e4a4909c3: extended display-p3; unclipped greys past white; a CGColor's grey cached; the sweep's traits on the probe (in a window); more than 512 hued colours each checked; neutral colours; the helper's comment. The fixed side's five launches per side (95/67 every launch) are attached to the round-2 brief and the sign-off ask.

## Brief

Code review, ROUND 1 of 3 (host change, iOS/tvOS), of ONE commit, `git show HEAD` (8470e38325) on branch x2/tint-grey off origin/main. Read-only: no checkout, restore, stash, reset or edits. The plan theme2 approved, with its amendment and conditions, is reproduced at the end.

The commit message explains it. In short: TintDimming.grey (host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift) greyed each hued colour on a dimmed control face by reading UIKit's dimmed tint from ONE static probe UIView. In the app that probe kept the first colour's dimmed tint, so a launch greyed every colour to the first one's grey (measured: five inks all -> 182 in one launch, all -> 37 in another). A new view per colour or re-dimming the probe faults there (inside UIKit's tint traversal / CA commit). UIKit's dimmed tint was measured to be exactly CGColor.converted(to: extendedGray) at alpha x0.8, so grey now converts with Core Graphics, keeps the colour's own alpha, greys a colour in its own space (textChannels' 9 numbers, or a CGColor) from that space, and keeps the bounded per-colour cache. A new permanent sweep test guards the formula.

Check, with file:line:
- correctness of grey([Double]) and grey(CGColor): the neutral rule (sRGB channels within 24) unchanged; the own-space path (codes 0 extended sRGB / 1 display-p3 / 2 extended linear sRGB, matching ExactCore Batch.swift textChannels: `c + [code] + v`, and a profile's colour `c + [0] + e`); the cache keys (4 sRGB numbers vs 5 own-space numbers) can't collide; the 512 bound; returned shapes (4 numbers) where callers expect them (Run.greyColors, dimmedWithTint, boxTrackGrey, SRGBColors.color);
- that nothing in grey touches a UIView or UIKit state now (it runs inside UIKit's tint traversal);
- threading/isolation (@MainActor enum, static lets);
- whether any caller relied on grey returning 9 numbers for own-space colours (before, it returned 4 too) or on the old rounded Int keys;
- the tests: TintDimmingSweepIOSTests (iOS || tvOS; the elevated trait iOS-only) is sound and not vacuous; the expected values come from new UIViews in a safe context; the either-order and 520-colour cases; the window-vs-detached case; the wide-gamut case (CGColor and nine numbers); TintDimmingIOSTests' p3 label expectation change is right;
- the conditions below are each met or say what's missing;
- the source-size caps (1500 lines a file) and nothing unrelated changed.
Give findings as BLOCKER / MAJOR / MINOR / NIT with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

Theme2's conditions (16:55 + 17:03): (1) the SWEEP XCTest (grey == a fresh UIView's dimmed tint, run where creating views is safe) stays in the suite permanently; it runs on tvOS too if the host builds there [note from the author: build.mjs --test runs only iOS simulators; the file compiles for tvOS]; (2) include wide-gamut (Display P3, out-of-sRGB) and translucent inputs in the sweep, and say how grey handles a colour that isn't sRGB (author: straight from its own space, which matches UIKit; via sRGB was 5 levels off for p3 red); (3) the x0.8 alpha factor a named constant with a comment citing the sweep, the colour's own alpha rule unchanged; (4) red-before proof = a 5-launch drive on both sides (handle ~95, icons ~67 each launch), numbers attached (author: main gives before 20/20 95/46 20/20 95/20 95/95, after 46/46 20/20 95/46 95/30 95/95; the fixed run is in progress); (5) increased contrast in the sweep; (6) cost measured (author: cold 18-21 us a colour, warm 0.7-1.1 us, old probe read 3.7-4.3 us), keep the simpler cache choice.

## Review

## MAJOR

**Code 1 greys Display P3 in the non-extended space, so components outside 0…1 are clamped.** `TintDimming.ownSpace` maps code 1 to `CGColorSpace.displayP3` at `host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:51`. The same code is extended Display P3 everywhere else that reads `textChannels`: `TextEngine.color` at `host/apple/Sources/ExactKit/Text.swift:119`, and `BatchValue.wideSpaces` at `host/apple/Sources/ExactKit/Batch.swift:63`. `push_wide` writes those components unclamped (`host/apple/src/style.rs:667-672`); the CSS parser does not clamp `display-p3` to 0…1 (`color/src/lib.rs:371-375`).

Core Graphics on this machine:

| components | space | stored | grey level |
|---|---|---|---|
| 1, 0, 0, 1 | displayP3 and extendedDisplayP3 | 1, 0, 0, 1 | 134.7205 |
| 1.2, 0, 0, 1 | displayP3 | 1, 0, 0, 1 | 134.72 |
| 1.2, 0, 0, 1 | extendedDisplayP3 | 1.2, 0, 0, 1 | 162.92 |
| 1, −0.2, 0, 1 | displayP3 | 1, 0, 0, 1 | 134.72 |
| 1, −0.2, 0, 1 | extendedDisplayP3 | 1, −0.2, 0, 1 | 128.66 |

A label takes `grey([Double])` (`Run.greyColors`, `textChannels`' nine numbers). A fill of the same colour takes `grey(CGColor)`, which converts the extended `CGColor` and keeps 162.92. In-range P3 matches on both paths (P3 red 134.7205, the commit's 134.72; sRGB red 129.80). Codes 0 and 2 already use `extendedSRGB` and `extendedLinearSRGB`.

Use `CGColorSpace.extendedDisplayP3` for code 1, and add one out-of-range Display P3 sample to the wide-gamut sweep.

**Condition (4) is unmet.** The approval asked for five launches on each side, with the fixed side at handle ~95 and icons ~67, numbers attached. The commit message says the fixed launches read 95 and 67 every time, and lists main as 20/20, 95/46, 95/20, 46/46, 95/95. The note with this review lists different pairs (before 20/20, 95/46, 20/20, 95/20, 95/95; after 46/46, 20/20, 95/46, 95/30, 95/95) and says the fixed run is still in progress. Both sequences still vary. Nothing in the tree holds the fixed-side five.

## MINOR

**The 520-colour case never reaches the cache bound on its own.** `TintDimmingSweepIOSTests.swift:63-71` builds 520 colours. Twelve have an sRGB spread of 24 or less, so `grey` returns them at `TintDimmingIOS.swift:30` and does not cache them. The other 508 are unique hued keys. The clear runs at `greys.count >= 512` (`TintDimmingIOS.swift:59`). From an empty cache the loop stops at 508, `removeAll` does not run, and the two assertions read the first grey still sitting in the cache. The either-order check just above (`:58-62`) is real: `[171,184,201]` and `[28,39,54]` are the two inks from the bug, and both orders are compared with a new view. Drive at least 512 hued colours through `grey`.

**`grey(CGColor)` stores an HDR grey through an sRGB initializer that clamps at 1.** `TintDimmingIOS.swift:45` calls `SRGBColors.color`, and `CGColor(srgbRed:)` at `Batch.swift:81` stores 1.5 as 1. Extended-sRGB red 2 converts to grey 266; linear red 8 converts to 328. `grey([Double])` returns that level unchanged. A neutral HDR colour still comes back as the original `CGColor` (spread check, covered by `testANeutralHDRFillKeepsItsRangeWhileDimmed`). Every in-range grey in the sweep is inside 0…255.

## NIT

**The new helper's comment is spliced.** `TintDimmingIOSTests.swift:890-891` reads "at UIKit's / UIKit's grey…". The `[Int]` overload at `:901` lost the comment this text used to document.

**The sweep's neutral branch does not run.** `TintDimmingSweepIOSTests.swift:45` continues when the spread is 24 or less. Every colour in `sweep` has a spread of at least 26, so that assertion is unreachable. Neutral HDR is covered by the older test.

## What checked out

- The neutral rule is unchanged: sRGB channels, spread greater than 24, otherwise the array is returned as it arrived, including nine numbers (`TintDimmingIOS.swift:30`).
- Own-space codes match `textChannels` (`Batch.swift:30-35`): 0 extended sRGB, 2 extended linear sRGB, and a profile colour is `c + [0] + e` in extended sRGB. Code 1 is the exception above. Nine-number components are passed through in unit range; the four-number path divides by 255.
- Cache keys are four sRGB numbers or five own-space numbers (`:32`). `Array` equality includes the count, so those keys cannot collide. Alpha is applied at the return (`[level, level, level, c[3]]`), so two alphas of one colour share a level safely.
- The 512 cap matches `SRGBColors`: clear, then insert (`:59-60`).
- Hued `grey([Double])` returns four numbers. `Run.greyColors` (`:240-247`), `dimmedWithTint` (`:149`), and `TextEngine.color` all take four or nine. `TextEngine.isShadow` takes 7 or 12, so a hued wide shadow (3+9 → 3+4) still paints. `boxTrackGrey` (`:233`) and `dimmedWithTint(CGColor)` (`:151`) get a `CGColor`. The previous `grey` also returned four (`Array(dimmed[0...2]) + [c[3]]`). The old rounded `Int` keys were private; nothing read them.
- `grey`, `ownSpace`, and `level` use `CGColor` and `CGColorSpace` only. No `UIView`, `UIColor`, or `tintAdjustmentMode`.
- `TintDimming` is `@MainActor`. `greys`, `srgb`, and `gray` are isolated statics. `SRGBColors` locks itself.
- The sweep file is `#if os(iOS) || os(tvOS)` and lives in the `ExactKitTests` directory target. `userInterfaceLevel` is inside `#if os(iOS)` (`TintDimmingSweepIOSTests.swift:40-42`). I did not compile tvOS. Expected greys come from a new `UIView` per call (`:21-28`), in the test process.
- The wide-gamut test (`:93-110`) covers a `CGColor` and nine numbers for in-range Display P3, including alpha 0.6. The sRGB sweep uses alphas 255, 204, 153, and 26.
- The P3 label expectation change is right. `TintDimmingIOSTests.swift:63` now asks UIKit for the grey of `displayP3(1,0,0)`, which is 134.72. The old sRGB-red grey is 129.80. `assertChannels` allows 2 levels (`:953`), so the old expectation would fail. The new helper reads a fresh view (`:892-898`).
- The window test (`:74-90`) compares a view in a window with a new detached view for three colours, one of them alpha 153. It checks the probe, and the other tests check `grey` against that probe.
- Line counts: `TintDimmingIOS.swift` 250, `TintDimmingIOSTests.swift` 956, `TintDimmingSweepIOSTests.swift` 112. The 1500 cap holds. The commit touches only these three files.

## Conditions

1. **Met.** The sweep is a permanent test, compiled for iOS and tvOS, with the elevated trait iOS-only. `build.mjs --test` stays iOS-simulator-only, as noted.
2. **Met for in-range P3 and for translucent inputs.** Own-space conversion matches UIKit there (P3 red was 5 levels off via sRGB). Out-of-range P3 on the nine-number path is the major finding.
3. **Met.** `uikitDimmedAlpha` is 0.8 (`TintDimmingIOS.swift:17-19`), the comment cites `TintDimmingSweepIOSTests`, and `grey` returns the colour's own alpha. The sweep checks both (`:47` and `:51`).
4. **Not met.** See the major finding.
5. **Met.** Increased contrast is in the trait list (`TintDimmingSweepIOSTests.swift:39`).
6. **Met as a choice.** The simple 512-entry dictionary is still there. The cold 18–21 µs / warm 0.7–1.1 µs figures are the author's; this review did not remeasure them.

VERDICT: CHANGES
