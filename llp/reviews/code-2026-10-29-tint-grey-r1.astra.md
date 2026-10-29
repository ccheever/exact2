# Code review: a dimmed control face's greys from Core Graphics (iOS host), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `8470e38325`.
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

- **MAJOR — [TintDimmingIOS.swift:51](/tmp/iosfix2-rv-tg1-astra/host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:51): Display P3 loses extended components.** Code 1 selects bounded `displayP3`, whereas [TextEngine.color](/tmp/iosfix2-rv-tg1-astra/host/apple/Sources/ExactKit/Text.swift:119) uses `extendedDisplayP3`. Core Graphics therefore clips negative or above-one components before greying. Reproduced here: P3 `[1.5, 0.2, 0, 1]` produces **140.45** through the new nine-number path versus **208.60** from its extended CGColor. Use `extendedDisplayP3` and include an extended-component sweep case.

- **MINOR — [TintDimmingSweepIOSTests.swift:22](/tmp/iosfix2-rv-tg1-astra/host/apple/tests/ExactKitTests/TintDimmingSweepIOSTests.swift:22): The trait sweep never applies its traits to the probe.** Creation, dimming and the `tintColor` getter run under the existing environment; only the resulting colour receives `resolvedColor(with:)`. This does not guard dimming decisions made from the view’s traits. Apply and assert the requested traits before dimming. Resolving a static colour returns that same colour. [Apple’s explanation](https://developer.apple.com/videos/play/wwdc2019/214/?time=1237)

- **MINOR — [TintDimmingSweepIOSTests.swift:64](/tmp/iosfix2-rv-tg1-astra/host/apple/tests/ExactKitTests/TintDimmingSweepIOSTests.swift:64): The eviction case does not independently exceed the cache bound.** Counting the generated inputs gives 508 hued colours and 12 neutrals. Including A/B, this test alone inserts at most 510 entries. Generate more than 512 distinct hued inputs and compare their results with fresh-view expectations; currently the loop discards every result except the revisited first colour.

- **MINOR — [TintDimmingIOS.swift:44](/tmp/iosfix2-rv-tg1-astra/host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:44): The CGColor overload bypasses the grey cache.** Previously it reached the cached array overload; now every hued call performs the grayscale conversion again. This affects `dimmedWithTint` and `boxTrackGrey`. `SRGBColors` caches only the resulting colour, after conversion. Preserve a colour-space-aware cache here, or separately qualify this uncached path; the quoted warm-cache timing does not establish its cost.

Otherwise, the neutral threshold and own-alpha rule are unchanged. Codes 0 and 2, including profile colours encoded as extended sRGB, match the producer. Four- versus five-element keys cannot compare equal; the implemented array-cache bound is 512. Return shapes remain compatible with all inspected callers, and none depends on rounded integer keys. Both overloads now avoid UIView/UIKit state; `@MainActor` protects the mutable cache. The ordinary P3 label expectation change is correct.

The sweep contains substantive comparisons against fresh UIViews in a safe XCTest context, including translucent and out-of-sRGB inputs. The either-order and window comparisons are meaningful, subject to the coverage gaps above.

| Approval condition | Review status |
|---|---|
| 1. Permanent sweep, including tvOS execution | Permanent test included with both platform guards. tvOS execution remains unproven; [the runner selects iOS](/tmp/iosfix2-rv-tg1-astra/host/apple/build.mjs:1476). |
| 2. Wide gamut, translucency, own-space handling | Included; extended P3 handling needs correction. |
| 3. Named 0.8 constant and preserved own alpha | Met. |
| 4. Five launches on both sides | Commit reports fixed 95/67 every launch, but does not attach the fixed five-launch results separately for both consumer sides. |
| 5. Increased contrast | Listed, but not installed on the probe; coverage incomplete. |
| 6. Measured cost and retained cache | Measurements reported; CGColor caching exception remains unqualified. |

Only the three relevant files changed: **250, 956 and 112 lines**, all below 1,500. `git diff --check` passed. Review remained read-only; I ran the Core Graphics reproduction and input-count check, but no builds or XCTests.

**VERDICT: CHANGES**
