# Review round 2, LLP 1121 stage 9 A1 (Apple draws icon paths), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING — [ExactCore/IconPath.swift:71](/tmp/rv-s9a-r2/host/apple/Sources/ExactCore/IconPath.swift:71): watchOS compilation breaks.** watchOS can import UIKit, but `UIGraphicsImageRendererFormat` and `UIGraphicsImageRenderer` are unavailable there. A read-only SDK compiler probe confirmed this. Moving the helper into ExactCore exposes the failure even for SF-only apps. Exclude the bitmap helper from watchOS; keep the shared path and transform functions available.

2. **SHOULD-FIX — [NodeViewIOS.swift:625](/tmp/rv-s9a-r2/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:625): centered icons retain the previously resized bitmap.** A 24-point icon enlarged into a 96-point box stays 96 points when `object-fit` changes to `none`. Likewise, a `scale-down` icon shrunk into a small box remains undersized when that box grows. The `.center` branch never restores `image`. Restore the intrinsic image there and test both transitions.

3. **SHOULD-FIX — [SnapshotLeaves.swift:264](/tmp/rv-s9a-r2/host/apple/Sources/ExactSnapshot/SnapshotLeaves.swift:264): widget paths ignore `object-fit` and the em-square sizing model.** A 24-point icon with `scale-down` in a 96-point box renders at 96 points. `cover` also behaves as contain. Fit the natural em square according to `fit`, then fit the viewBox inside that square, matching the page hosts. Test `scale-down`, `cover`, and a rectangular viewBox.

4. **SHOULD-FIX — [SnapshotLeaves.swift:267](/tmp/rv-s9a-r2/host/apple/Sources/ExactSnapshot/SnapshotLeaves.swift:267): overlapping paths compound translucent tint.** Each shape receives `ink` separately. At 50% opacity, overlapping paths become 75% opaque; a CoreGraphics reproduction produced alpha 128 outside the overlap and 192 inside. Compose the separately filled, opaque paths into one mask, then apply the tint once.

5. **NIT — [icons.rs:196](/tmp/rv-s9a-r2/contract/lower/src/icons.rs:196): the watchOS diagnostic misidentifies valid templates.** A valid template without `sf` reaches “is no one-colour shape ()” and “To fix it, ;”, then recommends a template again. Report the missing watchOS SF source explicitly and add a watchOS diagnostic assertion.

Round 1’s path separation, coordinate precision, tint fallback, formatting, and documentation fixes are present. Caps and `cargo fmt --all -- --check` passed. Cargo/native tests and the full tvOS build remain unverified here because the sandbox denies build/cache writes.
