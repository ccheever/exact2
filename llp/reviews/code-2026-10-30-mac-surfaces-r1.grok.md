# Code review: the Mac's card roles draw the under-page grey (kernel table), round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `6163743560`.
- **Method:** one brief (sha256 `67cf4f3374f4f11ad0a47c21d2ed65d884528e37030480e33ea1a34145fc2517`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean (nits).
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

## What the diff does

`git diff origin/main...HEAD` is nine files, +158/−4. In `kernel/tables/schema.json` the only edits are the 4th column of two rows:

- `secondary-background` at `schema.json:5894`: `controlBackgroundColor` → `underPageBackgroundColor`. Alias, `secondarySystemBackgroundColor`, and the four hexes stay `#f2f2f7` / `#1c1c1e` / `#f5f5f5` / `#1e1e1e`.
- `secondary-grouped-background` at `schema.json:5924`: the same name change. Hexes stay `#ffffff` / `#1c1c1e` / `#ffffff` / `#1e1e1e`.

`background` and `grouped-background` stay `windowBackgroundColor` (`schema.json:5884`, `schema.json:5914`). `tertiary-background` and `tertiary-grouped-background` stay `controlBackgroundColor` (`schema.json:5904`, `schema.json:5934`).

`underPageBackgroundColor` is an `NSColor` class property, `API_AVAILABLE(macos(10.8))`, in the macOS 27 SDK `NSColor.h:219`. The host floor is 14.0 (`host/apple/build.mjs:140`, `host/apple/intents.mjs:198`, the test build at `host/apple/build.mjs:1446`). `native_name_ok` (`kernel/src/style/roles.rs:596-607`) admits `[a-z][A-Za-z0-9]*Color`, which this name matches. The host lookup (`SystemColor.swift:139-147`, and the same check in `SnapshotStyle.swift:172-181`) requires a two-argument class method returning an object, which this property is. The header's "should not be used for drawing" line is the same kind of comment already on `windowBackgroundColor` (`NSColor.h:218`) and `controlBackgroundColor` (`NSColor.h:220`), which the table already paints.

## Who else keys on these names

`standsOnSheetGround` is `window is SheetWindow` and `sys == "windowBackgroundColor"` (`SheetsMac.swift:94-105`). `standsOnSplitGround` is the same name, unwritten, with the split active (`SplitMac.swift:654-658`). `BoxLayerMac.swift:89` nils the fill from those two predicates. A card whose `sys` is `underPageBackgroundColor` fails both.

Mac sources that mention `controlBackgroundColor` or a card name, and that this move leaves alone:

- `FrameHealthOverlayMac.swift:39` and `:72` assign `NSColor.controlBackgroundColor` directly.
- `TextAreaMac.swift:470` is `ThemeRoles.color("Field", …, or: .controlBackgroundColor)`.
- `SheetsMac.swift:109-111` matches `exactGroupedCardColor`. The grouped-card metric stays `-exact-platform-color(macos exactGroupedCardColor, …)` (`schema.json:6143`); a theme override swaps in the role id (`kernel/src/style/metrics.rs:175-187`), not the NSColor string.
- `GroupedListIOS.swift:1056` is the iOS file, and it asks `ThemeRoles` for `secondary-grouped-background` by role name. The `ios` column did not move.
- Alerts and the dialog surface set `BackgroundColor` to `-exact-secondary-grouped-background` (`kernel/src/arena/alert.rs:31-33`). On the Mac that role now paints under-page grey. That is the card change itself.

No Mac path matches the strings `secondary-background`, `secondary-grouped-background`, or the old card `sys` value.

## Other consumers of the Mac column

The Apple host emits `r.macos` only under `cfg!(target_os = "macos")` (`host/apple/src/style.rs:611-616`). No web path reads that field. The `macos` hit in `host/web-js/build.mjs:286` is `manifest.host.macos.resources`. Linux resolves these roles by name into its own materials (`host/linux/src/presenter/palette.rs:155-157`), so the NSColor string never reaches it.

`SnapshotStyle.named` maps `windowBackgroundColor` to SwiftUI `.background` (`SnapshotStyle.swift:143`). `underPageBackgroundColor` falls through to `platformColor` (`SnapshotStyle.swift:144`), the dynamic `NSColor` lookup `controlBackgroundColor` already used. A `.background` case would be the wrong mapping. None is required.

Mac `check` is unimplemented (`Agent.swift:367`). Contrast judgements use the Apple hex columns (`scripts/agent-check.mjs:22-23`, indices 4 and 5), which did not move, or captured pixels. The name map at `agent-check.mjs:18-19` is first-wins, so `underPageBackgroundColor` now records `secondary-background` and `controlBackgroundColor` now records `tertiary-background`. iOS reports UIColor names (`AgentCheckIOS.swift:414`). Nothing on the Mac reports an NSColor name as `fillRole` today.

`SCHEMA_DIGEST` is generated (`kernel/build.rs:301`). The only literal pin is `kernel/src/wire/codec.rs:732`, now `0x497dcbf7076a8a19`. I did not recompute the hash. Fixture plans are compiled at test time and embed the generated digest (`host/apple/xctest.mjs:13-18`).

## Tests and the line cap

`roles.rs:845-878` pins both card `macos` strings to `underPageBackgroundColor`, asserts each differs from its page, pins `grouped-background` to `windowBackgroundColor`, and pins the `ios` name plus light, dark, neutral-light, and neutral-dark. Those hexes match the table, alpha `ff` included.

`SurfacePairsMacTests` boots the compiled fixture (`scripts/fixtures/surface-pairs.contract`, wired at `host/apple/xctest.mjs:17`). The Mac test path writes `EXACT_SURFACE_PAIRS_PLAN` (`host/apple/build.mjs:1445`); the test reads that name (`SurfacePairsMacTests.swift:23`). `BatchValue.channels` goes through `SystemColor`, and `appKit()` (`SurfacePairsMacTests.swift:12-17`) reads `NSColor` in the same appearance on its own. The fixture fallbacks are the Apple hexes (`#f2f2f7` / `#1c1c1e` and `#ffffff` / `#1c1c1e`). Under-page is `#f6f6f6` / `#282828`, so a failed lookup fails the equality. The grouped card's light fallback is `#ffffff`, the same as the page, so a failed lookup also fails `XCTAssertNotEqual`.

The contrast iterations (`SurfacePairsMacTests.swift:31-41`) pass `contrast: true`, which selects `.accessibilityHighContrastAqua` and `.accessibilityHighContrastDarkAqua` (`SystemColor.swift:163-165`) and compares them with an AppKit read of that same appearance. `drawsHighContrast` returns nil on macOS because those named appearances resolve as plain Aqua (`SystemColor.swift:309-323`), and the commit's own measurements say window, control, and under-page do not move under increased contrast. The leg still fails if the name does not resolve. It repeats the aqua and darkAqua resolution check under the contrast flag. It does not show Increase Contrast changing a pixel.

The split and sheet tests inject the `sys` name by hand, so the fixture test is what binds the table. Their colour asserts compare against `NSColor.underPageBackgroundColor`, with fallbacks that are not that colour (`SplitMacTests.swift:273-280`, `VisibilityMacTests.swift:144-158`). Sample points in the sheet test sit on the card and the ground: the page is 240×160, the card is `(16, 16, 200×80)`, `NodeView` is flipped (`NodeViewMac.swift:25`), and this file already treats `colorAt` y = 0 as the top border (`VisibilityMacTests.swift:105-106`). `(100, 50)` is on the card; `(230, 150)` is on the ground.

The 1,500-line cap is per source file (`rules/RULES.md:16`, `scripts/caps.mjs:24-28` and `:147`). Touched sources: `roles.rs` 1417, `SplitMacTests.swift` 1258, `codec.rs` 1020, `VisibilityMacTests.swift` 273, `xctest.mjs` 277, `SurfacePairsMacTests.swift` 46, the contract 10. `schema.json` and `QUEUE.md` are outside that set. The new queue line (`QUEUE.md:3`) is 701 characters and is the dark-mode tertiary inversion the commit describes. The other eight files are that change, its digest, and its tests.

`llp/1095-platform-colours.rfc.md:252-254` still lists `controlBackgroundColor` for the two card roles. That RFC is the historical record, including open question 3 at `:853-862`, and this commit does not edit it.

## Findings

**NIT** — `host/apple/tests/ExactKitTests/VisibilityMacTests.swift:162`. The "apart from the ground" check is `abs(red delta) + abs(alpha delta) > 0.02`. The unpainted sheet ground is transparent (the sibling at `:127` expects alpha under 0.01), so the card's alpha of 1 satisfies it. Dark window grey (`#1e1e1e`) and under-page (`#282828`) also differ by about 0.04 in red, so the same check still passes if the ground painted the window colour opaquely. The colour of the card is the `fill[0]` compare at `:158`. The ground claim is `standsOnSheetGround` at `:152`, which is the predicate `BoxLayerMac.swift:89` already uses.

**NIT** — `host/apple/tests/ExactKitTests/SplitMacTests.swift:274`. `standsOnSplitGround` is the style just applied (unwritten `windowBackgroundColor`) plus membership in the live split. The sibling at `:254` asserts `layer?.backgroundColor == nil` for that ground. This test does not. The card's `cgColor` equality at `:279-280` is a real compare against AppKit.

**NIT** — `host/apple/tests/ExactKitTests/SurfacePairsMacTests.swift:31-41`. The increased-contrast iterations agree with AppKit under `.accessibilityHighContrastAqua` and `.accessibilityHighContrastDarkAqua`. `SystemColor.swift:311-312` records that those appearances resolve as plain Aqua, and these three colours are unchanged under increased contrast. The iterations prove the `contrast: true` lookup, and they do not prove a contrast change.

VERDICT: CLEAN
