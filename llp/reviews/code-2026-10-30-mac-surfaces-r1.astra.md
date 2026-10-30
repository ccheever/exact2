# Code review: the Mac's card roles draw the under-page grey (kernel table), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `6163743560`.
- **Method:** one brief (sha256 `67cf4f3374f4f11ad0a47c21d2ed65d884528e37030480e33ea1a34145fc2517`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes (minor).
- **Disposition:** the split and sheet card tests now prove the paint (layer fill; opaque card apart from the sheet's ground); the contrast legs' claim is qualified; LLP 1095's table and contract-for-agents name the under-page grey.

## Brief

Code review, ROUND 1 of 3 (kernel table change: the Mac's colour roles), of ONE commit, `git show HEAD` on x2/mac-surfaces off origin/main. Read-only: no checkout, restore, stash, reset or edits. The commit message explains it; theme2 approved the plan (~/x2theme/iosfix2-mac-surfaces-plan.md is not in the tree; its terms are in the message).

Check, with file:line:
- kernel/tables/schema.json: only the `macos` field (the 4th column: NSColor class property) of secondary-background and secondary-grouped-background changes, to underPageBackgroundColor; nothing else in the table moves. Is underPageBackgroundColor a valid AppKit class property returning NSColor on the deployment target (host/apple's macOS minimum)? Does roles.rs `native_name_ok` (or whatever vets names) admit it?
- That grouped-background staying windowBackgroundColor is right: SheetsMac.swift `groundIsTheWindows`/`standsOnSheetGround` and SplitMac.swift `standsOnSplitGround` key on that name; does anything ELSE on the Mac key on controlBackgroundColor or on the card roles' names (search host/apple/Sources for controlBackgroundColor, "secondary", exactGroupedCardColor, ThemeRoles names), so moving the cards to underPageBackgroundColor changes behaviour somewhere unintended (grouped list cells, sheets' grouped cards, fields, the Inspector, the frame-health card)?
- Every other consumer of the Mac column: the web (does any web path read `macos`?), Linux, the snapshot renderer (host/apple/Sources/ExactSnapshot/SnapshotStyle.swift maps some NSColor names to SwiftUI styles: does underPageBackgroundColor need a case there, or does it fall to a default that's wrong?), the agent's `check` contrast judgements on the Mac, tests or docs that list the Mac names.
- The schema digest literal in kernel/src/wire/codec.rs moves with the table; any other place that pins it.
- The tests: the kernel test (roles.rs) is sound and pins the Apple/neutral columns; SurfacePairsMacTests (a compiled fixture, scripts/fixtures/surface-pairs.contract, wired in host/apple/xctest.mjs) compares against AppKit resolved per appearance, independently of Exact's resolver; the split and sheet card tests prove the ground still stands on the split surface / sheet material and the card paints. Any vacuous assertion? Increased contrast on the Mac: SystemColor.swift notes AppKit's named high-contrast appearances resolve as plain Aqua — does the test's contrast leg prove anything?
- QUEUE.md's new line; nothing unrelated; line caps (1500).
Give findings as BLOCKER / MAJOR / MINOR / NIT with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No BLOCKER or MAJOR findings.

- **MINOR — Split test does not prove the card paints.** [SplitMacTests.swift:276](/tmp/iosfix2-rv-ms1-astra/host/apple/tests/ExactKitTests/SplitMacTests.swift:276) reads `cgColor`, which resolves the style independently of painting. It would pass with a transparent card layer. Assert the actual layer fill or captured pixels, alongside the transparent ground. Also unwrap both conversions at line 279: currently `nil == nil` can pass.

- **MINOR — Increased Contrast coverage is overstated.** [SurfacePairsMacTests.swift:31](/tmp/iosfix2-rv-ms1-astra/host/apple/tests/ExactKitTests/SurfacePairsMacTests.swift:31) exercises named appearances, not the system accessibility setting. An independent AppKit probe confirmed that those names resolve to ordinary Aqua/Dark Aqua, as [SystemColor.swift:309](/tmp/iosfix2-rv-ms1-astra/host/apple/Sources/ExactKit/SystemColor.swift:309) documents. These iterations establish lookup parity, but cannot establish actual Increased Contrast behaviour. Qualify the coverage claim or supply verification with the real setting.

- **NIT — Documented mappings are stale.** [LLP 1095:252](/tmp/iosfix2-rv-ms1-astra/llp/1095-platform-colours.rfc.md:252) and line 254 still specify `controlBackgroundColor`. [contract-for-agents.md:4187](/tmp/iosfix2-rv-ms1-astra/docs/contract-for-agents.md:4187) also describes the grouped card as white without qualifying the platform. Update the mappings and qualify that description.

The remaining checks look sound:

- Only the two fourth-column cells change, at [schema.json:5894](/tmp/iosfix2-rv-ms1-astra/kernel/tables/schema.json:5894) and [5924](/tmp/iosfix2-rv-ms1-astra/kernel/tables/schema.json:5924). The kernel test pins both Apple and neutral columns.
- [Apple documents the NSColor class property](https://developer.apple.com/documentation/appkit/nscolor/underpagebackgroundcolor). The installed SDK declares availability from 10.8, below the host’s [14.0 minimum](/tmp/iosfix2-rv-ms1-astra/host/apple/Package.swift:109); [native_name_ok](/tmp/iosfix2-rv-ms1-astra/kernel/src/style/roles.rs:596) admits it.
- Retaining `windowBackgroundColor` preserves [sheet](/tmp/iosfix2-rv-ms1-astra/host/apple/Sources/ExactKit/Mac/SheetsMac.swift:94) and [split](/tmp/iosfix2-rv-ms1-astra/host/apple/Sources/ExactKit/Mac/SplitMac.swift:654) recognition. Grouped-list cards use the separate [exactGroupedCardColor metric](/tmp/iosfix2-rv-ms1-astra/kernel/src/style/metrics.rs:261). Fields, Inspector and frame-health paths introduce no additional dependency on the changed names.
- [SnapshotStyle.swift:144](/tmp/iosfix2-rv-ms1-astra/host/apple/Sources/ExactSnapshot/SnapshotStyle.swift:144) dynamically resolves the NSColor; no additional case is needed. Web rendering and Linux retain their existing columns. The shared agent checker reads Mac aliases, but [Mac `check` is unsupported](/tmp/iosfix2-rv-ms1-astra/host/apple/Sources/ExactKit/Agent.swift:367).
- The compiled fixture is wired correctly; its oracle calls AppKit independently. The [sheet test captures painted output](/tmp/iosfix2-rv-ms1-astra/host/apple/tests/ExactKitTests/VisibilityMacTests.swift:159).
- Independently recomputed digest: `0x497dcbf7076a8a19`, matching [codec.rs:732](/tmp/iosfix2-rv-ms1-astra/kernel/src/wire/codec.rs:732). No additional current pin found. QUEUE’s addition is relevant; no unrelated changes or source-line-cap violations.

Read-only review; no files changed. Swift/kernel suites were not rerun. Caps’ source scan passed, but its Cargo subcheck was blocked by filesystem restrictions.

**VERDICT: CHANGES**
