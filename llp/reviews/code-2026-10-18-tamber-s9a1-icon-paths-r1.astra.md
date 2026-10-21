# Review round 1, LLP 1121 stage 9 A1 (Apple draws icon paths), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING — [host/apple/src/icon_path.rs:18](/tmp/rv-s9a-r1/host/apple/src/icon_path.rs:18): concatenation is not a path union.** Separate SVG paths with opposite winding cancel where they overlap when combined into one nonzero fill. A reversed inner square therefore becomes a hole, although Linux and SVG paint it solid. Confirmed with CoreGraphics. Preserve each path’s boundary and fill them separately into the opaque template mask. Add an overlapping, opposite-winding pixel test.

2. **SHOULD-FIX — [host/apple/src/icon_path.rs:45](/tmp/rv-s9a-r1/host/apple/src/icon_path.rs:45): three-decimal rounding can erase valid icons.** With `viewBox="0 0 .0004 .0004"`, a square spanning that box has every coordinate serialized as zero. SVG units have no fixed minimum size. Use round-trip `f32` formatting and test a small viewBox.

3. **SHOULD-FIX — [IconPath.swift:69](/tmp/rv-s9a-r1/host/apple/Sources/ExactKit/IconPath.swift:69): UIKit enlarges a bitmap rendered at font size.** A 16-point icon displayed in a 96-point box is stretched sixfold. Bounds and display scale are absent from `symbolKey`, so resizing never regenerates it. Render at the fitted display size and scale, retaining the em-square intrinsic size, or draw the path as a vector. The current tests inspect image metadata and tint properties, not pixels, so they miss this.

4. **SHOULD-FIX — [contract/lower/src/icons.rs:186](/tmp/rv-s9a-r1/contract/lower/src/icons.rs:186): lint relaxation exceeds renderer support.** This admits SVG-only icons for watchOS too, while [SnapshotLeaves.swift:240](/tmp/rv-s9a-r1/host/apple/Sources/ExactSnapshot/SnapshotLeaves.swift:240) still draws only a nonempty `symbolName`. These icons pass validation and render blank there. Keep the source restriction for unsupported renderers until A2 supplies their path rendering.

5. **SHOULD-FIX — [tamber_icons.rs:81](/tmp/rv-s9a-r1/contract/cli/tests/it/tamber_icons.rs:81): formatting check fails.** The pinned rustfmt reports changes for the assertions at lines 81 and 86. Format this file before landing.

6. **NIT — [llp/1121-tamber.rfc.md:3786](/tmp/rv-s9a-r1/llp/1121-tamber.rfc.md:3786): the claimed documentation update is incomplete.** D10 still describes Apple as SF-only, and line 3124 still carries “Apple draws only an icon’s SF Symbol.” Update those statements and F3’s Apple paragraph to describe template fallback and the remaining non-template restriction.

Caps passed; every changed source file is below 1,500 lines. Read-only CoreGraphics checks confirmed findings 1–2. Native tests and the tvOS build were not rerun in this sandbox.
