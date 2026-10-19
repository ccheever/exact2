# Review round 2, LLP 1121 stage 9 A2 (Apple's control faces draw icon paths), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING — Grouped path icons lose an authored `AccentColor`.** [GroupedListIOS.swift:932](/tmp/rv-s9a2-r2/host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:932) treats a nil configuration tint as unauthored. But line 899 deliberately leaves it nil when the symbol follows `@tint`. The new fallback therefore paints that path in the label colour instead of the cell’s accent. **Fix:** distinguish an absent tint from `followsTint`; preserve the cell’s tint for the latter. Extend `testASymbolInTheAccentTakesTheCellsTint` with a path icon and a changing accent.

2. **SHOULD-FIX — Projected tab bars retain the wrong image size after resizing.** [SegmentsIOS.swift:379](/tmp/rv-s9a2-r2/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:379) invalidates items only when their count, title or symbol changes. Path size now depends on horizontal size class, so an iPad moving between compact and regular widths retains its previous 23- or 17-point bitmap, even after `sync()`. **Fix:** include the resolved image size in the cache identity and refresh on relevant trait changes. Test compact → regular → compact without changing the faces.

3. **SHOULD-FIX — Agent output can name a different icon than the author wrote.** [icon_path.rs:72](/tmp/rv-s9a2-r2/host/apple/src/icon_path.rs:72) returns the first declaration matching the resolved source. Two used tokens naming the same SVG—such as `icon.reply` and `icon.respond`—therefore both report the first name. This misses Theme2’s “as written” ruling. **Fix:** preserve the chosen name through source expansion and populate `symbolIcon` from that identity. Test two identical-source declarations through the bridge, including a computed source.

The other round 1 fixes look consistent on inspection. Caps and diff-whitespace checks passed. Cargo tests were blocked by the read-only sandbox creating `/tmp/rv-target-astra`; Swift UI tests were not run. No files changed.
