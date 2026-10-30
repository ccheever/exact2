# Code review: the Mac's card roles draw the under-page grey (kernel table), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `355a41e1c7`.
- **Method:** one brief (sha256 `5d5ae6e177864bc409b37d3e6da05e4d15790e49ff6157c306f8a242cd590a7b`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes (minor).
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

No BLOCKER or MAJOR findings.

- **MINOR — Sheet test still does not prove the ground remains unpainted.** [VisibilityMacTests.swift:159](/tmp/iosfix2-rv-ms2-astra/host/apple/tests/ExactKitTests/VisibilityMacTests.swift:159). The revised RGB comparison meaningfully proves that an opaque card stands apart from its background; removing the external colour-resolution comparison is reasonable. However, `standsOnSheetGround` only checks classification. If [BoxLayerMac.swift:89](/tmp/iosfix2-rv-ms2-astra/host/apple/Sources/ExactKit/Mac/BoxLayerMac.swift:89) stopped suppressing the ground’s fill, these assertions would still pass: opaque window grey and under-page grey differ by approximately `10/255`, exceeding `0.02`. Assert the page layer’s absent background colour after display, as the split test now does. Round 1’s sheet-ground concern remains unresolved.

The other round 1 findings are resolved:

- [SplitMacTests.swift:274](/tmp/iosfix2-rv-ms2-astra/host/apple/tests/ExactKitTests/SplitMacTests.swift:274) checks actual layer paint, the unpainted ground, and unwraps both colour conversions.
- [SurfacePairsMacTests.swift:6](/tmp/iosfix2-rv-ms2-astra/host/apple/tests/ExactKitTests/SurfacePairsMacTests.swift:6) correctly limits the contrast claim to named-appearance lookup parity.
- [LLP 1095:252](/tmp/iosfix2-rv-ms2-astra/llp/1095-platform-colours.rfc.md:252) and [contract-for-agents.md:4187](/tmp/iosfix2-rv-ms2-astra/docs/contract-for-agents.md:4187) accurately describe the revised mappings.

The remaining checklist is sound: only the two Mac cells change; availability and name validation admit the property; grouped-list metrics and other platform consumers retain their behavior; snapshot lookup handles it dynamically; fixture wiring and kernel assertions are correct. I independently recomputed `0x497dcbf7076a8a19` and checked changed-source line caps and diff whitespace.

Read-only; no files changed. Swift/kernel suites were not rerun.

**VERDICT: CHANGES**
