# C2 Apple (x2/c2-apple 88f40b2646 + 81eabe2a46), round 1, Astra (gpt-6-astra, max)

**WRONG**

1. **Rule-only updates leave stale rendering.** [NodeViewMac.swift:990](/tmp/iosfix-rv-c2a1/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:990) and [NodeViewIOS.swift:1121](/tmp/iosfix-rv-c2a1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1121): setting or clearing only `symbolFillRule` never calls `updateSymbol()`. With unchanged source and style, macOS retains the previous layer rule and iOS retains its cached image. **Fix:** refresh on this prop’s set/clear. Test both directions through `Presenter.apply`; remove the manual refresh at [IconPathTests.swift:182](/tmp/iosfix-rv-c2a1/host/apple/tests/ExactKitTests/IconPathTests.swift:182), which masks the failure. The iOS branch never tests the transition.

2. **Nit: nonzero cache keys change unconditionally.** [NodeSymbolMac.swift:22](/tmp/iosfix-rv-c2a1/host/apple/Sources/ExactKit/Mac/NodeSymbolMac.swift:22) and [NodeViewIOS.swift:538](/tmp/iosfix-rv-c2a1/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:538) insert an extra colon even when the rule is absent. This violates the requested unchanged-key invariant, though I found no resulting pixel regression. **Fix:** add a discriminator only for even-odd icons, preserving the previous nonzero string exactly.

3. **Requested pixel coverage is incomplete.** [IconPathTests.swift:176](/tmp/iosfix-rv-c2a1/host/apple/tests/ExactKitTests/IconPathTests.swift:176) samples the backing image, [FaceSymbolTests.swift:63](/tmp/iosfix-rv-c2a1/host/apple/tests/ExactKitTests/FaceSymbolTests.swift:63) samples a standalone face image, and [SnapshotDriverTests.swift:632](/tmp/iosfix-rv-c2a1/host/apple/tests/ExactKitTests/SnapshotDriverTests.swift:632) uses an integer origin. Only macOS has the six-origin comparison. **Fix:** add fractional-origin rendering checks for the iOS leaf, a mounted face and the snapshot; exercise rule changes and verify that removing the iOS key discriminator fails.

I found no additional draw site dropping the rule. Grouped rows, toolbars, menus, tabs, widgets and Live Activities reach the updated helpers. Rust’s shared parsing, name matching, nonzero JSON and conservative wire-size allowance look correct.

Read-only review; `git diff --check` passed. Tests and mutations were not executed—this checkout has no built artifacts.
