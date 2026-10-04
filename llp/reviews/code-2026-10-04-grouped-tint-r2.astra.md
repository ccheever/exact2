# Code review, round 2: grouped list (iOS) symbol tint (461e80f95), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `461e80f95`.
- **Method:** one brief (sha256 `4beca76a9aa20d4f095c3d0a6fdd8a13c1ed67a6108e239f9946b8d157e5ea9d`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (landed):**
  - 1 (the test's row would be custom): taken. A title sits between the symbol and the chevron, the initial configuration is unwrapped, and a style-only tint change is asserted in both appearances.
  - 2 (string comparison): taken. `looks` holds `BatchValue?`.

---

Appearance, symbol selection, and background removal are resolved. The test finding is partially resolved. I found no new production correctness defect.

1. **Should-fix — The test still supplies a tree the kernel would classify as custom.** [GroupedListIOSTests.swift:187](/tmp/x9-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:187)

   The children remain hidden image → symbol → chevron, with **no title**. After removing the accessory and symbol, the kernel finds no text and resets the row to custom ([grouped.rs:198](/tmp/x9-review/kernel/src/grouped.rs:198)). The stubbed standard-row model conceals that mismatch.

   The test nevertheless provides useful coverage: it obtains a cell before the update, detects selection of the hidden red image, and distinguishes dynamic tint from a fixed colour by resolving both appearances. It exercises an absent-to-present tint change, but never edits an existing symbol’s tint through a style-only batch.

   **Fix:** add a nonempty title between symbol and chevron, consistent with the model. After obtaining the configured cell, change only the symbol’s tint and assert the replacement colour in both appearances. Unwrap the initial content configuration explicitly so its absence cannot satisfy `XCTAssertNil`.

2. **Nit — Tint equality still uses debug strings despite `BatchValue` being equatable.** [GroupedListIOS.swift:448](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:448)

   Every visited row formats its tint into a string for comparison. This adds avoidable allocations and makes value comparison depend on textual formatting.

   **Fix:** store and compare the `BatchValue` directly, representing absence explicitly. The map’s lifecycle is otherwise sound: replacement drops removed IDs, insertion configures new rows, and retained rows reconfigure when their tint changes.

The remaining round-1 dispositions check out:

- **Appearance:** resolved at [GroupedListIOS.swift:352](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:352). The provider uses its supplied traits. Capturing `value` is safe: `BatchValue` contains value types, and the closure retains neither the view nor the presenter. Later style changes replace the provider through `looks`. Destructive and disabled colours still take precedence.
- **Which symbol:** resolved for kernel-produced standard rows at [GroupedListIOS.swift:441](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:441). Hidden children are skipped. `sf/` names are compared directly; role names deliberately rely on the kernel’s mapping through `apple_symbol`, rather than incorrectly comparing a role with its Apple name ([grouped.rs:80](/tmp/x9-review/kernel/src/grouped.rs:80)). Removing the trailing accessory cannot change the leading child of a valid symbol-and-title row; degenerate rows become custom and are rejected by the guard.
- **Background removal:** resolved. The combined diff contains no background-configuration change, and [§6.1:202](/tmp/x9-review/llp/1084-native-grouped-lists.rfc.md:202) explains why the underlay remains.
- **Misplaced comment:** resolved; the carry description again precedes `carry`.

Static review only; no files changed or tests run.

Verdict: LAND WITH FIXES
