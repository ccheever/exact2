# Code review: a dimmed control face's greys from Core Graphics (iOS host), round 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `9ee79c10b8`.
- **Method:** one brief (sha256 `e359f1bc2834ef6591137ce2a9f93e214eed28e10d86bb57528fcb11248ac119`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes (minor).
- **Disposition:** Fixed in dfd8349bbd: the neutral judgement on the untagged colour (a tagged near-neutral HDR colour stays), and the tagged-first case on a cold colour. A narrow re-check by both follows.

## Brief

Code review, ROUND 3 of 3 (host change, iOS/tvOS plus one shared Text.swift line), of branch x2/tint-grey: `git log -p origin/main..HEAD -- host` in this worktree (8470e38325 the fix; 4e4a4909c3 round 1; d5e1eb6f0b round 2; the review records are docs). Read-only: no checkout, restore, stash, reset or edits. Rounds 1 and 2 are in llp/reviews/code-2026-10-29-tint-grey-r{1,2}.{astra,grok}.md, each with its disposition.

Review the WHOLE change fresh, with the round-1 checklist (in those briefs), and whether each round-2 finding is resolved. Round 2's headroom findings were resolved by measurement in the other direction from Astra's suggestion: on an iPhone simulator UIKit's dimmed tint of an extended-sRGB [2, 0.5, 0.2] is 281.97 whether the colour is tagged with ColorRange.tagged's headroom or not, while Core Graphics' grey of the tagged colour tone-maps it to 142.21. So grey converts the colour untagged on both paths (matching UIKit, theme2's rule), the CGColor key leaves the tag out on purpose, and a grey past white from the CGColor path is then tagged for its layer. Check that reasoning and its tests (testATaggedHDRColourGreysAsUIKitGreysIt).

Evidence from the author: the dark post-menu, handle/icons grey on screen, five launches per side: main: before 20/20  95/46  20/20  95/20  95/95; after 46/46  20/20  95/46  95/30  95/95. With the fix: before 95/67  95/67  95/67  95/67  95/67; after 95/67  95/67  95/67  95/67  95/67 (8470e38325; rounds 1-2 don't change these in-range sRGB inks' greys). iOS: TintDimmingSweepIOSTests, TintDimmingIOSTests, TextPaintTests, HDRColorTests, NativeButtonsIOSTests pass 2 of 2. Mac: TextPaintTests and HDRColorTests pass 2 of 2.
Give findings as BLOCKER / MAJOR / MINOR / NIT with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

**A tagged near-neutral HDR colour is greyed on the CGColor path.** `grey(CGColor)` classifies the colour from `c` converted to non-extended sRGB at `host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:47`, then greys `untagged(c)` at `:51`. Tagging leaves the components and the space name unchanged, and Core Graphics' sRGB conversion of the tagged colour tone-maps it. On this Mac, extended sRGB `[1, 1, 1.25]` has untagged sRGB spread 0 and extended-grey **259.47**, while the same colour tagged at headroom 1.596 has sRGB spread **47.6**. The spread test treats it as hued and returns an extended grey past white. The nine-number path decides from the sRGB clip in `c[0...2]` (`:31`), spread 0, and returns the colour unchanged, so a dimmed face's fill (`BatchValue.cgColor` tags, then this overload) and its text diverge. Extended Display P3 `[1, 1, 1.2]` does the same (untagged sRGB spread ~0, tagged spread 42.3). `testATaggedHDRColourGreysAsUIKitGreysIt` uses `[2, 0.5, 0.2]`, whose untagged and tagged sRGB spreads are 204 and 229, so the gate never sees this case.

Run the spread test on `untagged(c)`. Add a sweep case that tagged extended-sRGB `[1, 1, 1.25]` stays as it arrived, and that its nine-number form does too.

## Round 2

- **Layer tag missing (Grok, minor).** Resolved. A grey past white is `ColorRange.tagged` at `TintDimmingIOS.swift:54`. For untagged extended-sRGB `[2, 0.5, 0.2]` the grey is **281.965**; tagging that grey sets `contentHeadroom` **1.258**, and a relative-colorimetric read of the tagged grey in linear Rec. 2020 is still **1.258**, so `ColorRange.isHDR` stays true. The sweep asserts `contentHeadroom > 1` (`TintDimmingSweepIOSTests.swift:174`).
- **HDR stroke dropped (both, minor).** Resolved. `PresentedText.attributes` accepts width plus nine at `Text.swift:58` and paints through `TextEngine.color`, which tags. `testAStrokeInItsOwnSpaceIsPainted` checks the width and extended component 1.2.
- **Cache key omits headroom (Astra, major).** Resolved by the measurement, in this direction. Tagged and untagged `[2, 0.5, 0.2]` share space name `kCGColorSpaceExtendedSRGB` and components `[2, 0.5, 0.2, 1]`. Converting the tagged colour to `extendedGray` yields **142.212**; `untagged` (`:79`) rebuilds the colour from its space and components, `contentHeadroom` goes to 0, and the grey returns to **281.965**. The key leaves the tag out on purpose (`:85`), and both orders store that same level. A space outside the six named ones gets a nil key (`:88`) and is converted every time, so two unnamed spaces with the same components no longer share an entry.
- **Array path greys a different HDR colour than the painters (Astra, major).** Resolved for a hued colour. The nine-number path builds an untagged `CGColor` from the wide components (`:36`). The `CGColor` path greys `untagged(c)`. Both give **281.965** for `[2, 0.5, 0.2]`. The result past white is tagged afterwards for the layer. The sweep's `testATaggedHDRColourGreysAsUIKitGreysIt` (`:163`) is the right contract: UIKit of the plain colour, UIKit of the tagged colour, and `grey` of both, plus the result's headroom. It locks the 281-versus-142 mistake, because a grey that tone-mapped the tag would miss UIKit. The residual is the neutral gate above, which that sample does not reach.

## What checked out

The neutral rule on the array path is the sRGB clip, spread greater than 24, and a neutral nine-number colour comes back unchanged (`:31`). Codes match `textChannels` (`Batch.swift:30-35`): 0 extended sRGB, 1 extended Display P3, 2 extended linear sRGB, and a profile colour is extended sRGB. In-range hued results stay four numbers. Past white, the nine are `[255, 255, 255, alpha, 0, L, L, L, alpha]` with `L` in 0... past 1, which is the layout `TextEngine.color` reads (`Text.swift:119-121`).

Cache keys stay disjoint. Four sRGB numbers, five own-space numbers, and a `CGColor` key of `[-1e9, space, components...]`. Known spaces are indices 0...5. A two-component colour is not one of those RGB spaces, so it is not cached. Alpha is not part of the stored level. The cap is still 512, clear then insert (`:71`), shared with `SRGBColors`.

`grey` and `level` use `CGColor` and `CGColorSpace` only. `TintDimming` is `@MainActor`. Callers: `Run.greyColors` keeps a stroke at 5 or 10 and a shadow at 7 or 12 (`:276-277`); `TextEngine.isShadow` allows 7 or 12; `dimmedWithTint`, `boxTrackGrey`, and `CssAnimation.greyed` take the `CGColor` overload; `TextEngine.color` takes four or nine. An in-range sRGB grey stays at most 255 (a spread-25 near-white measures 253.6), so those inks still return four numbers.

The sweep reads each expectation from a new view, with traits set on a probe in a window. Either order uses the two inks from the bug. The 600-colour loop is all hued, asserts 600 distinct keys, and checks the first again after the bound. Wide gamut covers a `CGColor` and nine numbers, including extended components and alpha 0.6. The P3 label expectation is UIKit's grey of `displayP3(1, 0, 0)`.

`#abb8c9` greys at **182.32**, and `182.32 × 0.52 = 94.81`, reported as 95. `#6f839f` greys at **128.88**, and `128.88 × 0.52 = 67.02`, reported as 67. Later commits do not change an in-range sRGB grey. Line counts: `TintDimmingIOS.swift` 280, `Text.swift` 362, `TintDimmingIOSTests.swift` 956, `TintDimmingSweepIOSTests.swift` 179, `TextPaintTests.swift` 642. The branch range touches those five host files. `git diff --check` is clean.

I reproduced the Core Graphics numbers on this Mac. I did not re-run the simulator suites or the five launches.

VERDICT: CHANGES
