I found one blocker and two should-fix issues. I reviewed `e7b87b391` first and read all five commits.

1. **Blocker — a fitted sheet can stop growing when rows arrive.** [host/apple/src/layout.rs:277](/private/tmp/bsky4-rv/wt-detent/host/apple/src/layout.rs:277)

   The measurement uses children’s heights after layout inside the current sheet. With CSS’s default `flex-shrink: 1`, adding content can shrink the children instead of increasing the measured extent. [CSS Flexbox](https://www.w3.org/TR/css-flexbox-1/#flex-shrink-property)

   Concrete regression case: take the existing Rust fixture and call `host.resize(390.0, 164.0)` before pressing More—the height its initial three 44-point rows request. Adding the fourth row distributes those rows into the existing 132-point content area; the measured height remains 164 instead of growing to 208. The current test adds rows while the viewport is still 844 points tall, hiding this feedback.

   This contradicts §9.11’s claim that the extent follows the rows independently of the sheet. Its documented exclusions omit shrinking children.

   **Fix:** measure preferred content height independently of the sheet’s vertical constraint, or explicitly require nonshrinking children and enforce/document that restriction. Add the regression above.

2. **Should-fix — round 3 introduces incorrect percentage-padding measurement for hidden routes.** [host/apple/src/layout.rs:260](/private/tmp/bsky4-rv/wt-detent/host/apple/src/layout.rs:260), [layout.rs:272](/private/tmp/bsky4-rv/wt-detent/host/apple/src/layout.rs:272)

   Padding percentages are resolved against `node.frame.width`, rather than the containing block’s width as CSS requires. [CSS Box Model](https://www.w3.org/TR/css-box-3/#padding-physical)

   Example: a 200-point-wide route inside a 400-point containing block, with `padding-bottom="10%"` and one nonshrinking 44-point row. Correct content height is **84**; `fitted_size` calculates **64**. Before round 3, an `overflow="hidden"` route retained Taffy’s correct extent through `content_size`; it now discards that vertical extent. Visible routes have the same problem; scrolling routes retain Taffy’s measurement in this example. Rotation changing the containing width compounds the discrepancy.

   **Fix:** use the kernel’s resolved padding with the host cover excluded, or resolve percentages against the actual containing block. Test hidden, visible, and scrolling variants with unequal parent/route widths.

3. **Should-fix — the tests do not exercise a sheet following content changes.** [host/apple/tests/ExactKitTests/ModalDetentIOSTests.swift:23](/private/tmp/bsky4-rv/wt-detent/host/apple/tests/ExactKitTests/ModalDetentIOSTests.swift:23)

   The XCTest manually invokes a resolver backed by a number variable. Removing the new `modals.contentChanged(v)` call from `PresenterIOS.swift:1081` would leave the added tests passing, although a presented sheet would lose content-change invalidation. Neither the resolver’s cover subtraction nor actual sheet geometry is exercised.

   **Fix:** present a sheet through the real presenter and test content growth/shrinkage, keyboard show/hide, rotation, and Dynamic Type. Include hidden/scroll/visible routes and `"fit-content large"`, preserving the selected detent.

The round-3 overflow distinction otherwise agrees with Swift’s scroll creation, and its added clipping assertion would catch the previous bottom-cover error. Literal mixed-detent refusals and diagnostics match §9.11; bound values remain unchecked as documented. macOS, Linux, and both web targets ignore detents consistently with the LLP.

These are source-level findings, not simulator reproductions. `git diff --check` passed; the tree remains unchanged. I did not build or run Rust/UIKit suites under the read-only restriction.

**Finding 1 blocks landing.**