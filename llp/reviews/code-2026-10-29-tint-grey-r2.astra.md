# Code review: a dimmed control face's greys from Core Graphics (iOS host), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `c70be42f51`.
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

- **MAJOR — [TintDimmingIOS.swift:73](/tmp/iosfix2-rv-tg2-astra/host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:73): The CGColor cache key can collide.** It omits content headroom. In a read-only Core Graphics reproduction, extended-sRGB `[2, 0.5, 0.2, 1]` converts to grey **281.965** untagged and **142.212** with the headroom `ColorRange.tagged` assigns (**3.179845**). Both receive exactly the same key; whichever arrives first answers for both. Separately, all unnamed spaces use `""`: two calibrated RGB spaces with identical components produced **128.417** and **60.476**, again sharing a key. [Apple documents that color-space names can be nil](https://developer.apple.com/documentation/coregraphics/cgcolorspace/name). Include the actual space identity and headroom in cache equality, with regression cases for both collisions.

- **MAJOR — [TintDimmingIOS.swift:35](/tmp/iosfix2-rv-tg2-astra/host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:35): The array path greys a different HDR representation from production painters.** It constructs an untagged color, whereas [TextEngine.color](/tmp/iosfix2-rv-tg2-astra/host/apple/Sources/ExactKit/Text.swift:119) and [BatchValue.cgColor](/tmp/iosfix2-rv-tg2-astra/host/apple/Sources/ExactKit/Batch.swift:55) apply `ColorRange.tagged`. Even with empty caches, the example above therefore yields **281.965 for text versus 142.212 for a fill**. Extended-P3 `[1.5, 0.2, 0, 1]` similarly yields **208.600 versus 137.765**. The [wide/HDR sweep](/tmp/iosfix2-rv-tg2-astra/host/apple/tests/ExactKitTests/TintDimmingSweepIOSTests.swift:136) constructs untagged inputs, so it misses this production mismatch. Use the painters’ representation consistently and test through those producers.

- **MINOR — [TintDimmingIOS.swift:40](/tmp/iosfix2-rv-tg2-astra/host/apple/Sources/ExactKit/IOS/TintDimmingIOS.swift:40): The new nine-number result is incompatible with the text-stroke caller.** `Run.greyColors` prepends the stroke width, producing ten numbers, but [PresentedText.attributes](/tmp/iosfix2-rv-tg2-astra/host/apple/Sources/ExactKit/Text.swift:57) accepts only `st.count == 5`. A stroke whose dimmed grey exceeds white is consequently omitted. Extend that consumer and assert the resulting stroke attributes. The other inspected color consumers, including seven-/twelve-number shadows, accept the new shape.

Round-1 disposition: extended-P3 clamping, trait installation, insufficient eviction inputs, unreachable neutral coverage, the helper comment, and missing five-launch evidence are resolved. CGColor caching and above-white preservation were implemented, but the defects above prevent considering those paths complete.

Otherwise, the neutral threshold and own-alpha rule remain intact. Neither grey overload touches UIView/UIKit state; `@MainActor` protects the cache. The key *formats* are disjoint, and the 512 bound holds. The sweep has 600 distinct hued inputs, each checked against UIKit. Only three source files and two review records changed; source lengths are **266, 956, and 160**, below the cap. The diff whitespace check passed.

Review remained read-only. Numerical reproductions used this Mac’s Core Graphics; I did not rerun UIKit suites or launches. Your iPhone results are supplied evidence; tvOS execution remains unproven.

**VERDICT: CHANGES**
