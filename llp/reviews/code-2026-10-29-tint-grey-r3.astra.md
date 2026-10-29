# Code review: a dimmed control face's greys from Core Graphics (iOS host), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `9ee79c10b8`.
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

- **MINOR — [TintDimmingSweepIOSTests.swift:170](/tmp/iosfix2-rv-tg3-astra/host/apple/tests/ExactKitTests/TintDimmingSweepIOSTests.swift:170): The tagged-first cache miss is never tested.** Both permutations share one key, which the first untagged call populates. Consequently, removing `untagged(c)` would still pass all four comparisons. A read-only Core Graphics reproduction confirmed **281.965** throughout that sequence, versus **142.212** for a cold tagged conversion. Ensure each permutation starts with an absent key, using distinct inputs or eviction. The test’s independent UIKit comparison and output-headroom assertions remain meaningful.

The round-2 production findings are resolved: untagged conversion matches the supplied UIKit measurement; omitting headroom from the key is therefore appropriate; unknown spaces bypass caching; above-white CGColors receive headroom tags; and the shared stroke painter accepts width plus nine channels.

The remaining checklist checks out: neutral and alpha rules, extended-space mappings, disjoint cache keys, the 512-entry bound, caller return shapes, main-actor isolation, and no UIKit state access during conversion. The sweep covers installed traits, neutrals, wide gamut, translucency, and 600 distinct hued inputs. All five changed source files meet the line cap; branch changes are confined to those files and four review records. Whitespace checks passed.

The supplied launch evidence resolves round 1’s measurement requirement. I did not rerun XCTest suites; iOS/macOS passes are author evidence, and tvOS execution remains unverified. The worktree is unchanged.

**VERDICT: CHANGES**
