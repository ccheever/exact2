# Code review: grouped list (iOS), symbol tint and custom background (ec39f0ee7), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `ec39f0ee7`.
- **Method:** one brief (sha256 `77757fe3c5cb882df810a8d0b255e0fd6dba0f10f3f677c06d78bc5f1af50d3e`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):**
  - 1 (appearance): taken. The tint is a dynamic UIColor that resolves the pair per trait collection.
  - 2 (first image vs the model's symbol): taken. The lookup is the first shown child, and it must be an image of the model's symbol.
  - 3 (clearing the cell is not sheet parity): taken. The background change is dropped; §6.1 says why.
  - 4 (the test): taken. The new test has a hidden image, the symbol and a chevron, uses a light-dark pair and resolves both appearances.

---

1. **Should-fix — Symbol colours become stale after an appearance change.** [GroupedListIOS.swift:447](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:447), [GroupedListIOS.swift:357](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:357).

   `NodeView.color` correctly resolves `light-dark()` using the image’s current appearance, but returns a fixed `UIColor`. `looks` compares the raw pair, which stays unchanged when appearance changes. The image’s trait callback repaints its hidden view without reconfiguring the list cell. Consequently, a black/light–white/dark symbol can remain black in dark mode. This also affects the sheet’s default symbol tints.

   **Fix:** supply a dynamic colour, or cache resolved channels and explicitly trigger reconfiguration on appearance changes, including changes without a style batch.

2. **Should-fix — The first image can differ from the kernel’s selected symbol.** [GroupedListIOS.swift:444](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:444).

   The kernel filters `display: none` before identifying the leading symbol ([grouped.rs:152](/tmp/x9-review/kernel/src/grouped.rs:152)); this lookup does not. With a hidden red image followed by a visible blue symbol and title, UIKit displays the visible symbol in red. Updating the visible symbol’s tint also leaves `looks` unchanged.

   A normal leading symbol plus trailing chevron works. A leading `when` also works: the runner flattens active branch roots into the parent’s children; there is no branch view requiring recursive traversal.

   **Fix:** carry the selected symbol’s view ID in the kernel model and use it for both configuration and caching, or mirror the kernel’s visible-child selection precisely.

3. **Should-fix — Clearing the cell changes background composition relative to D7.** [GroupedListIOS.swift:345](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:345).

   The sheet puts its default background on the **group**, not individual rows ([grouped.rs:404](/tmp/x9-review/contract/lower/src/grouped.rs:404)). Thus, the presence check is not accidentally matching that sheet default. However, a transparent authored row reveals the group’s cell colour on the web; it does not erase its parent’s background. Clearing the iOS cell reveals the list background instead. Translucent fills and uncovered row margins also differ.

   **Fix:** retain the default cell underlay to match existing sheet composition. If transparent rows must remove the card, change the sheet’s background ownership in the same patch and update D7; §6.1 currently claims parity that the implementation does not provide.

4. **Nit — The new test proves configuration assignment, but not the claimed update behaviour.** [GroupedListIOSTests.swift:183](/tmp/x9-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:183).

   The model is stubbed as a standard row, while the authored row contains only an image—something the real kernel classifies as custom. The test also does not obtain the initial cell before making changes, so it does not reliably exercise reconfiguration of an existing cell. `XCTAssertNotEqual(..., .clear)` can pass for a nil configuration.

   **Fix:** use a kernel-consistent symbol/title/accessory tree; obtain the cell first, then change only its tint. Add hidden-image and appearance cases, and check background removal, custom-to-standard reuse, and resolved highlight/selection backgrounds. A compiled-sheet case should establish the transparency semantics.

The `looks` map’s row-set handling is sound: replacement drops removed IDs, inserted rows configure normally, and retained rows compare by ID. Its added cost is linear in rows and inspected children, with avoidable string allocations. Use an equatable colour value instead of `BatchValue`’s textual description.

The UIKit background APIs themselves are appropriate: standard rows reset the configuration on reuse, and UIKit automatically updates default backgrounds for selection/highlight state. [Apple documentation](https://developer.apple.com/documentation/uikit/uicollectionviewcell/automaticallyupdatesbackgroundconfiguration)

Static review only; no files changed or tests run.

Verdict: LAND WITH FIXES
