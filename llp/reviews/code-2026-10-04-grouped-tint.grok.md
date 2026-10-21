# Code review: grouped list (iOS), symbol tint and custom background (ec39f0ee7), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `ec39f0ee7`.
- **Method:** one brief (sha256 `77757fe3c5cb882df810a8d0b255e0fd6dba0f10f3f677c06d78bc5f1af50d3e`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** 1 (a hidden image), 2 (appearance), 3 (the test) and 4 (the misplaced comment) are all taken. The background part is dropped after astra's finding 3.

---

## Findings

1. **Should-fix** — A hidden image is treated as the leading symbol. `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:445`

`symbolView` returns the row container's first `image` subview. The kernel's symbol is the first child that is actually shown, after a trailing accessory has been removed (`kernel/src/grouped.rs:152-196`). `display: none` does not remove a child from the subview list, so that image is the one whose `tint_color` is read. A row that hides one image and then shows the real symbol keeps the hidden image's tint, or UIKit's accent when the hidden image has no `tint_color`. A trailing chevron is a later sibling, so a leading symbol plus a chevron still resolves to the symbol. A `when` in first place is the same list the kernel walks: the runner inserts only the active arm's roots as the row's children (`runner/src/instance/region.rs:282-284`).

Fix: skip children whose `style["display"]` is `"none"`, and take the first remaining image whose symbol source is `row.symbol` (strip `symbol:` and `sf/`, as `apple_symbol` does). Use that view for both `symbolView` and `tint(of:)`.

2. **Should-fix** — The symbol colour does not follow appearance. `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:357`

`symbol.color("tint_color", .label)` resolves `light-dark()` through `channels(dark: drawsDark)` (`NodeViewIOS.swift:762-767`) into `TextEngine.color`, a fixed sRGB `UIColor` (`Text.swift:724-727`). `looks` stores the raw style value (`GroupedListIOS.swift:447-448`). An appearance change does not change that value and does not call `update`, and nothing on the list observes the trait change. The sheet writes `tint-color` on every leading symbol as `light-dark(#0088ff, #0091ff)` (`contract/lower/src/grouped.rs:23` and `:744`), so this path runs for every symbol row, author's override included. After a scheme change the icon keeps the previous side of the pair. Destructive and disabled still follow the appearance: they assign `.systemRed` and `.tertiaryLabel` afterwards (`GroupedListIOS.swift:361-368`).

Fix: set `imageProperties.tintColor` to a dynamic `UIColor` that reads `symbol.style["tint_color"]?.channels(dark:)` from the traits passed into the provider, and keep the `looks` reconfigure so a later style edit replaces that provider. Leave the destructive and disabled assignments after it.

3. **Should-fix** — The new test does not prove the claim in its comment. `host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:183`

`groupedList` is stubbed, so the kernel's row reader never runs. The tree has one image and a fixed `[0, 0, 0, 255]`. That stays green with a hidden image in front, a chevron sibling, a `when` arm, or a `light-dark()` pair, and it never changes the window's appearance. The nil tint on row 10 (`:198`) is a model symbol with no image child. In the app the sheet still puts `tint-color` on that image, so the assertion does not show that an unauthored symbol keeps UIKit's tint. The second `apply` does reconfigure when a tint appears. It does not reconfigure when only the appearance changes, which is the case `looks` misses.

Fix: drive a row whose children are a `display: none` image, then the symbol, then a chevron, with a `light-dark()` tint. Assert the symbol's colour, override the window to the other appearance, and assert the other side without a further style op.

4. **Nit** — `carry`'s comment now heads `symbolView`. `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:441`

The paragraph describes the carried row, and the added sentence says the symbol is "the row's first image", which is the lookup in finding 1. Move the original comment back onto `carry`, and describe `symbolView` as the shown image the model named.

## Checked, no defect

The group's cell colour is not on the row. The sheet sets `background-color` on the group column (`contract/lower/src/grouped.rs:404`); `row()` does not (`:577-596`). `background_color` is not inherited (`kernel/tables/schema.json`, bit 47), and the host sends only rows the node itself sets. `style["background_color"] != nil` is the author's attribute or class (or a background animation), which is the right switch. `.clear()` has no state styling, so a custom row that paints its own background does not pick up the list highlight. That matches the declared "custom row takes no cell highlight". `defaultBackgroundConfiguration()` is the system list fill and still updates for highlight and selection. Both branches assign `backgroundConfiguration` on every `configure`, and the cell registration runs again on reuse, so a cell moved between a clear custom row and a standard row does not keep the other fill.

`looks` is rebuilt from the current snapshot's ids, so removed rows drop out and new ids are configured by insertion. A tint edit on an image still reaches `update`, because the touched set walks up to the list. The walk is one string per row of a settings list.

Verdict: LAND WITH FIXES
