# Code review: the Mac's card roles draw the under-page grey (kernel table), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `de51e5df92`.
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

No BLOCKER / MAJOR / MINOR / NIT findings.

All earlier findings are resolved:

- [SplitMacTests.swift:274](/tmp/iosfix2-rv-ms3-astra/host/apple/tests/ExactKitTests/SplitMacTests.swift:274) checks the unpainted ground and the card’s actual layer fill against AppKit, unwrapping both colour conversions.
- [VisibilityMacTests.swift:152](/tmp/iosfix2-rv-ms3-astra/host/apple/tests/ExactKitTests/VisibilityMacTests.swift:152) now checks the ground’s absent layer fill after display and the card’s present fill. Together with the opaque card pixel and captured colour difference, this meaningfully tests the intended separation. Colour identity is independently covered by the split and compiled-fixture tests; removing the external-resolution comparison is reasonable.
- [SurfacePairsMacTests.swift:8](/tmp/iosfix2-rv-ms3-astra/host/apple/tests/ExactKitTests/SurfacePairsMacTests.swift:8) correctly limits increased-contrast coverage to named-appearance lookup parity.
- [LLP 1095:252](/tmp/iosfix2-rv-ms3-astra/llp/1095-platform-colours.rfc.md:252) and [contract-for-agents.md:4187](/tmp/iosfix2-rv-ms3-astra/docs/contract-for-agents.md:4187) accurately describe the revised mappings.

The fresh checklist pass found no unintended consumer changes: only the two Mac colour cells change; availability, name validation, sheet/split recognition, grouped-list metrics, snapshot resolution and fixture wiring remain sound. Other platforms retain their columns.

Independently recomputed digest: `0x497dcbf7076a8a19`, matching the pin. Changed-source line caps and diff whitespace checks pass.

Read-only; no files changed or Swift/kernel suites rerun. The reported 301-class Mac pass predates the revised assertions.

**VERDICT: CLEAN**
