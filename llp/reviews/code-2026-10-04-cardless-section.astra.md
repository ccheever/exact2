# Code review: a grouped-list section without its card (50cf88e90), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `50cf88e90`.
- **Method:** one brief (sha256 `09173566aca7581698b841497c4dcee4d18c147484a7140f703d132e75ca6b30`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r2):** 1 taken (tvOS guard). 2 taken by narrowing: only the literal `transparent` is admitted, so no coloured or one-sided card can disagree across hosts. 3 taken: no row separators or grouped borders on the sheet for a card-less section. 4 taken: a configuration update handler restores UIKit's highlight for a pressable standard row. 5 taken: a `class` beside the attribute is refused; contract-for-humans and §6.2 say what is admitted. 6 taken: a standard row through both transitions, and its highlight. DEFERRED: separator assertions on iOS (the layout's configuration is not inspectable from the test).

---

Static review of HEAD; no builds or tests run.

1. **Blocker — tvOS compilation breaks.** [GroupedListIOS.swift:260](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:260) assigns `showsSeparators` inside a file compiled for both iOS and tvOS. The installed UIKit SDK declares this property `API_UNAVAILABLE(tvos)` in `UICollectionLayoutList.h:79`. **Fix:** guard the assignment with `#if !os(tvOS)`.

2. **Should-fix — the Boolean loses supported background semantics.** [kernel/src/grouped.rs:135](/tmp/x14-review/kernel/src/grouped.rs:135), [commands.rs:265](/tmp/x14-review/host/apple/src/abi/commands.rs:265), [GroupedListIOS.swift:362](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:362).
   
   `background-color="#ff0000"` produces a red card on the sheet hosts but a default UIKit card on iOS. `light-dark(transparent, #ffffff)` produces `card=true` in both appearances, so iOS retains its card in light mode. The OR matches §6.2’s definition, but that definition creates platform disagreement. Also, `background_color=None` represents `currentcolor`, which can itself resolve to transparent.
   
   **Fix:** carry the effective background colour, including both appearance branches, into the native model. Resolve it for the active appearance, honour its colour, and refresh backgrounds and separator visibility when the colour or appearance changes. Colour changes must reconfigure cells even when the Boolean remains unchanged.

3. **Should-fix — transparent sections retain separators on web, macOS and Linux.** [grouped.rs:434](/tmp/x14-review/contract/lower/src/grouped.rs:434) changes only the group background. Rows still receive their separator at [grouped.rs:600](/tmp/x14-review/contract/lower/src/grouped.rs:600), and `listStyle="grouped"` retains its outer borders at [grouped.rs:415](/tmp/x14-review/contract/lower/src/grouped.rs:415). A transparent section containing multiple rows therefore differs from iOS.
   
   **Fix:** suppress the sheet’s default row separators and grouped outer borders using the same effective card predicate, including runtime and appearance changes. Preserve row geometry and explicit author overrides.

4. **Should-fix — `.clear()` removes standard rows’ selection feedback.** [GroupedListIOS.swift:362](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:362). An enabled standard button remains selectable, but its background configuration now has no default styling, including highlighted/selected styling. Automatic configuration updates do not restore defaults to an empty configuration. [Apple’s background-configuration documentation](https://developer.apple.com/documentation/uikit/uibackgroundconfiguration-swift.struct).
   
   **Fix:** use a state-dependent configuration: clear at rest for a card-less row, and the default configuration updated for highlighted/selected state when the row is pressable. Custom rows should retain their existing authored feedback; disabled, inert and non-button rows should remain non-highlighting.

5. **Should-fix — moving the attribute before class expansion breaks precedence.** [grouped.rs:378](/tmp/x14-review/contract/lower/src/grouped.rs:378). For `section class=Blue background-color="transparent"`, removing the inline attribute means [class.rs:21](/tmp/x14-review/contract/lower/src/class.rs:21) no longer suppresses the class background. The outer section becomes blue and shows through the transparent group. A class-only transparent background also leaves the default card intact.
   
   **Fix:** resolve class/inline precedence before routing the winning background declaration to the group. If class backgrounds are intentionally unsupported, reject them explicitly instead. Update [contract-for-humans.md:969](/tmp/x14-review/docs/contract-for-humans.md:969); the RFC alone does not satisfy the repository’s author-guide requirement. Its “literal attribute” wording also needs correction: the implementation moves **any directly written expression**, not just literals.

6. **Should-fix — the new UIKit test does not exercise `recarded`.** [GroupedListIOSTests.swift:248](/tmp/x14-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:248) checks custom row 21. Custom rows are already unconditionally reconfigured at [GroupedListIOS.swift:292](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:292), so removing the new `recarded` condition would leave this test passing.
   
   **Fix:** test an otherwise unchanged **standard** row through both card transitions, and assert separator changes as well as background changes. Add coverage for selected/disabled states, multiple-row transparent sections, mixed `light-dark()`, coloured backgrounds, class precedence, and a bound background expression changing through the compiler/kernel/JSON path.

The ordinary opaque ↔ fully transparent runtime path otherwise looks sound: the kernel rereads current styles, group style updates reach the list’s ancestor-based refresh, and `recarded` requests reconfiguration and layout invalidation. Per-section `showsSeparators` is valid on iOS; UIKit supports list configuration at section scope. [Apple’s list-configuration documentation](https://developer.apple.com/documentation/uikit/uicollectionlayoutlistconfiguration-swift.struct?changes=_4).

`when` wrappers preserve scope through `over()`, and moved expressions remain intact. LLP 1091 resolves module names before lowering, so I found no additional module interaction. The MSRV concern is clear: [rust-toolchain.toml:2](/tmp/x14-review/rust-toolchain.toml:2) pins Rust 1.97.0, and `is_none_or` already appears elsewhere.

Verdict: DO NOT LAND
