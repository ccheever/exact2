# Code review: a dimmed control face's greys from Core Graphics (iOS host), round 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `c70be42f51`.
- **Method:** one brief (sha256 `8f50e753e1b104b415c085dec26cc3686894dd00a60c5baee965ce0572c3b392`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** Fixed in d5e1eb6f0b. The headroom findings were resolved the other way from Astra's suggestion, by measurement: UIKit's dimmed tint ignores a colour's headroom tag (281.97 tagged or not), so grey converts the colour untagged on both paths, and the CGColor key, which leaves the tag out, gives both forms the same (correct) grey. A grey past white from the CGColor path is tagged for its layer; a nine-number stroke is painted.

## Brief

Code review, ROUND 2 of 3 (host change, iOS/tvOS), of branch x2/tint-grey: `git log -p origin/main..HEAD -- host` in this worktree (8470e38325, then 4e4a4909c3 fixing round 1; the review records are docs). Read-only: no checkout, restore, stash, reset or edits. Round 1's two reviews are in llp/reviews/code-2026-10-29-tint-grey-r1.{astra,grok}.md.

Review the WHOLE change fresh (not only the round-1 fixes), with the same checklist as round 1 (in those records' briefs): correctness of TintDimming.grey([Double]) and grey(CGColor) in host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift; no UIView/UIKit state touched; cache keys can't collide (4 sRGB numbers, 5 own-space numbers, and the CGColor key [-1e9, space, components...]); the 512 bound; return shapes (4 numbers, or 9 in extended sRGB for a grey past white) against every caller (Run.greyColors, dimmedWithTint, boxTrackGrey, TextEngine.color, SRGBColors); the tests in TintDimmingSweepIOSTests.swift and TintDimmingIOSTests.swift are sound and not vacuous (traits now placed on the probe in a window with traitOverrides and checked; 600 hued colours past the bound; extended display-p3; HDR); the 1500-line cap; nothing unrelated changed.
And whether each round-1 finding is resolved.

Evidence from the author (theme2's condition (4), the red-before proof, the clone's dark post-menu, the handle's grey / the action icons' grey on screen, five launches per side):
- on main's TintDimming (the gate's SDK): before 20/20  95/46  20/20  95/20  95/95; after 46/46  20/20  95/46  95/30  95/95
- with this change (8470e38325 on the same SDK base; round 1 changes only own-space/HDR/CGColor paths, not these sRGB inks): before 95/67  95/67  95/67  95/67  95/67; after 95/67  95/67  95/67  95/67  95/67
- 95 is #abb8c9's UIKit grey 182 under the sheet's 0.52 dim; 67 is #6f839f's 128.9 likewise.
Suites: TintDimmingSweepIOSTests, TintDimmingIOSTests, NativeButtonsIOSTests pass 2 of 2 on an iPhone simulator.
Give findings as BLOCKER / MAJOR / MINOR / NIT with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

## MINOR

**A grey past white handed to a layer is not tagged with its exposure.** `TintDimming.grey(CGColor)` builds that color with a plain extended-sRGB `CGColor` at `host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:52`. `TextEngine.color` and `BatchValue.cgColor` both pass the same kind of color through `ColorRange.tagged` (`Text.swift:121`, `Batch.swift:55`). On iOS 26 that tag is `contentHeadroom`; `HDRColorTests` requires it on every HDR color a layer is given (`HDRColorTests.swift:23`, `:457`). Fills, borders, box shadows, and `boxTrackGrey` take this `CGColor` overload (`NodeView.cgColor` at `NodeViewIOS.swift:1012`, `FlatLeavesIOS.swift:406`, `BoxShadow.swift` after `dimmedWithTint`). The sweep converts components and never reads `contentHeadroom`, so it stays green either way. Wrap this color in `ColorRange.tagged` before returning it.

**A text stroke whose grey is past white is dropped while the face is dimmed.** The new array return at `TintDimmingIOS.swift:40` is nine numbers once the level is over 255. `Run.greyColors` keeps the stroke width and appends that grey (`TintDimmingIOS.swift:263`), so the stroke is ten numbers. The only painter accepts five (`Text.swift:57`: width plus four channels). A shadow survives the same return: three plus nine is twelve, and `TextEngine.isShadow` allows twelve (`TextRunPaint.swift:58`), then `TextEngine.color` paints the extended grey. An in-range grey is still four numbers, so those strokes stay five and paint. Teach that painter to take width plus nine, the same shape `Spec.headroom` already reads (`DynamicRange.swift:288`).

## Round 1

- **Extended Display P3 (both, major).** Code 1 is `extendedDisplayP3` (`TintDimmingIOS.swift:58`). On this machine, extended P3 `[1.5, 0.2, 0, 1]` greys at **208.60**; the bounded `displayP3` clip of those components greys at **140.45**. The sweep now includes `[1.5, 0.2, 0, 1]` and `[-0.1, 0.9, 0.3, 0.8]` and compares both the `CGColor` and the nine-number form with a fresh view (`TintDimmingSweepIOSTests.swift:134-146`).
- **Traits never reached the probe (Astra, minor).** `uikit` puts the probe in a window and sets `traitOverrides` before dimming (`TintDimmingSweepIOSTests.swift:29-38`). A second probe in that window is checked for dark and increased contrast (`:63-70`).
- **The bound case never passed 512 hued keys (both, minor).** The loop is 600 colours, the set size is asserted, each is hued (blue is 231...255 and red is at most 199, so the spread is at least 32), and each is compared with UIKit, then the first again (`:92-100`).
- **`grey(CGColor)` skipped the cache (Astra, minor).** It now stores through `level` under `cgKey` (`:50`, `:73-76`). The sweep reads the second colour twice (`:102-108`).
- **HDR grey clamped at 1 (Grok, minor).** Levels above 255 return nine extended-sRGB numbers, or an extended-sRGB `CGColor`, instead of `SRGBColors.color` / `CGColor(srgbRed:)`. The remaining holes are the two minors above.
- **Spliced helper comment (Grok, nit).** The two `uikitGrey` helpers each have their own comment again (`TintDimmingIOSTests.swift:890-901`).
- **Neutral branch unreachable (Grok, nit).** The sweep includes `[128, 128, 128]`, `[20, 30, 40]`, and `[250, 240, 230]` (`TintDimmingSweepIOSTests.swift:19`), and the neutral arm compares them with the input (`:74`).
- **Five launches (Grok, major).** The brief's pairs match the formula. `#abb8c9` is `[171, 184, 201]`, extended-grey **182.32**, and `182.32 × 0.52 = 94.81`, reported as 95. `#6f839f` is `[111, 131, 159]`, extended-grey **128.88**, and `128.88 × 0.52 = 67.02`, reported as 67. Fixed side 95/67 on all five launches, main still varying. Those launches are on `8470e38325`. The later commit does not change an in-range sRGB grey: both inks are under 255, so they still return four numbers.

## What checked out

`grey([Double])` still treats a spread of 24 or less on the first three channels as neutral and returns the array unchanged, including nine numbers (`TintDimmingIOS.swift:31`). Codes match `textChannels` (`Batch.swift:30-35`): 0 extended sRGB, 1 extended Display P3, 2 extended linear sRGB, and a profile colour is `c + [0] + e`. Hued in-range results stay four numbers. Past white, the nine are `[255, 255, 255, alpha, 0, L, L, L, alpha]` with `L` in 0... past 1, which is the layout `TextEngine.color` reads (`Text.swift:119-121`).

Cache keys do not collide. A four-number sRGB key and a five-number own-space key differ in count, and `Array` equality includes the count. A `CGColor` key is `[-1e9, space, components...]` (`:73-76`). Known spaces are indices 0...5. A two-component color makes a four-number key and a three-component color a five-number key, and both start with `-1e9`, which no sRGB channel or space code is. Alpha is not part of the stored level; the return uses the caller's own alpha, so two alphas of one colour share a level only when the key omits alpha. Here the key includes it, which costs entries and does not change the grey. The cap is still 512, clear then insert (`:66-67`), shared with `SRGBColors`.

`grey` and `level` use `CGColor` and `CGColorSpace` only. No `UIView`, `UIColor`, or `tintAdjustmentMode`. `TintDimming` is `@MainActor`. Callers of four or nine numbers: `Run.greyColors` for ink, background, underline, and strike; `dimmedWithTint([Double])`; `TextEngine.color`. `dimmedWithTint(CGColor)` and `boxTrackGrey` receive a `CGColor`. `channels()` is four numbers, so a fill's array path cannot exceed 255 after the bounded sRGB clip. `SRGBColors.color` is used only when the level is at most 255.

The sweep's expected greys come from a new view per call. Either order uses the two inks from the bug. The window test compares a view in a window with a detached one. The wide-gamut case covers a `CGColor` and nine numbers, including alpha 0.6. The P3 label expectation in `TintDimmingIOSTests.swift:63` is UIKit's grey of `displayP3(1, 0, 0)`.

Line counts: `TintDimmingIOS.swift` 266, `TintDimmingSweepIOSTests.swift` 160, `TintDimmingIOSTests.swift` 956. The 1500 cap holds. Past `origin/main`, the only files outside `host` are the two round-1 review records.

Conditions: (1) the sweep is permanent and compiled for iOS and tvOS; `userInterfaceLevel` stays inside `#if os(iOS)`. (2) Wide gamut, extended components, and translucency are in the sweep, taken from the colour's own space. (3) `uikitDimmedAlpha` is 0.8, the comment cites the sweep, and the return keeps the colour's alpha. (4) Met by the numbers above. (5) Increased contrast is set on the probe. (6) The 512-entry dictionary remains, and the `CGColor` path now uses it.

I did not re-run the simulator suites. The grey levels above were computed here with Core Graphics.

VERDICT: CHANGES
