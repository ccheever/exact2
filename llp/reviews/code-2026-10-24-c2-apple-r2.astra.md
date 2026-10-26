# C2 Apple (x2/c2-apple, Swift + Rust + round 1 fixes), round 2, Astra (gpt-6-astra, max)

**RIGHT WITH NITS.** No correctness bug found.

- **[FaceSymbolTests.swift:136](/tmp/iosfix-rv-c2a2/host/apple/tests/ExactKitTests/FaceSymbolTests.swift:136)** — The toolbar test checks only center alpha. Its helper redraws the item’s image at an integral origin; it neither samples the band nor captures fractional placement of the mounted face. Ignoring even-odd would fail, but a completely transparent even-odd toolbar image would pass. **Fix:** assert an inked band alongside the clear hole, and capture a face at a fractional origin.

The remaining checks look correct:

- Both platforms handle `symbolFillRule` being set or cleared through `applyProps`; the changed key regenerates the image or updates existing shape layers.
- Absent-rule keys, nonzero face tokens, JSON, and drawing operations retain their previous behavior.
- Grouped rows, toolbars, menus, tabs, widgets, and Live Activities reach rule-aware renderers.
- `icon_name` uses `Icon::of_row`, including rule equality. No additional raw Apple reader surfaced. The wire-size allowance covers the new property.
- Node and snapshot samples hit the band and hole; forcing nonzero would fail their assertions. The macOS comparison covers six origins at 1× and 2×.

An in-memory CoreGraphics probe confirmed the fractional node samples distinguish even-odd `[255, 0]` from nonzero `[255, 255]` band/center alpha. Rust/XCTest suites and source mutations were not run: this read-only checkout has no build artifacts. No files changed.
