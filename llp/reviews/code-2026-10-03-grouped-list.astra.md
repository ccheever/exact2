# Code review: LLP 1082, the native grouped list (origin/main..cc5d28641), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `cc5d28641`.
- **Method:** one brief (sha256 `542ffb5020084e4a6ebf1434d1bde925b6d1a116ce6a5c9735b10254487b2147`), the same one sent to grok; round 1; blind to the other review. Requested by the coordinator for Charlie. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** each finding checked in the source. Ten are fixed in `25a5b053c`; #3 is fixed for the agent and argued for the rest.
  1. *Cached switch target.* Fixed. The switch's action reads the row's current target when it fires (`flip(id, …)`). Tested in `testASwitchFollowsItsControlsStateAndTarget`, where a replaced control is the one flipped.
  2. *Accessory disabled state.* Fixed. `refresh` runs on every `mount` and sets the switch's `isEnabled` from the row and its current control. `detail(_:)` checks the current button's `disabled` and `inert`. Tested: a control disabled and then re-enabled with the row unchanged, and the agent's tap refused while it is disabled.
  3. *Scroll plumbing.* The agent's wheel now scrolls the collection view (`GroupedCollectionView`, `Agent.scroll`), driven on the simulator: rows below the fold were then tapped. Authored `scrollTop` writes and `scroll` events are argued and declared (LLP 1082 §4). The kernel's content height is the sheet's, not UIKit's, so mirroring one offset onto the other is wrong at both ends. A settings screen needs neither; a consumer that does gets it designed then.
  4. *Agent geometry and occlusion.* Fixed. The generic viewport check is skipped for nodes a grouped list draws (`draws`). `activate` resolves the cell, refuses one outside the list's port, and hit-tests the cell's middle through the window. The grouped check now runs before the control host's, so a toggle's tap goes through the same checks. Tested: scrolled away, a row's tap is refused and nothing is pressed.
  5. *Hidden sections and rows.* Fixed. `Kernel::grouped_list` skips `display: none` sections, rows and parts. Tested.
  6. *Nested views as subtitles.* Fixed. The stack must be a flex column. A `row` of two texts stays custom. Tested.
  7. *Classes under the sheet.* Fixed. Sheet rows are marked (`ua:`) and taken out before class expansion (`grouped::split`), so the order is sheet, class, attribute. Tested with a class's `min-height`.
  8. *Conditional leading symbols.* Fixed. The row's `margin-left` is a choice on the leading `when`'s condition, matching `part`. Tested in both states. `part` now reads `match` arms as it reads `when`. A first-place `each` is styled as no symbol, which is consistent on both sides.
  9. *Restore order.* Fixed. Rows go back last-carried first. Tested with three custom rows around a standard one.
  10. *Header and footer contents.* Fixed. A header or footer must be exactly one `text` (refused otherwise). Tested.
  11. *Plain gaps.* Fixed. A plain list's sections have no margins. Tested by layout.

---

1. **Must-fix — Cached switches retain the original target.** [GroupedListIOS.swift:301](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:301)  
   When a `when` replaces a row’s toggle with another control, the switch survives because it is cached by row ID. Its action still captures the first `target`, while its displayed state comes from the replacement. Tapping therefore addresses the removed control or the wrong control. Resolve the current target at activation, or recreate the switch when its target changes. Add a test replacing the accessory within a surviving row.

2. **Must-fix — Accessory disabled state is not consistently enforced.** [GroupedListIOS.swift:249](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:249), [GroupedListIOS.swift:295](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:295)  
   Changing only a toggle control’s `disabled` prop leaves the row model equal, so reconfiguration is skipped; `mount()` refreshes only `checked`. An enabled switch stays enabled and can dispatch input after becoming disabled. Detail accessories also check the row’s disabled state, never their own button’s. Refresh accessory enabled state every relevant batch and validate the current target before dispatch. Test both disabling and reenabling, plus an initially disabled detail button.

3. **Must-fix — The projected collection bypasses the list’s scroll plumbing.** [GroupedListIOS.swift:237](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:237)  
   Only insets are copied. Authored `scrollTop` writes still update the hidden `owner.scroll`, and collection scrolling never forwards the list’s `scroll` event. Additionally, `Agent.scroll` accepts only Exact’s `ScrollView`, so wheel operations skip this `UICollectionView`. Route scroll writes, callbacks, and agent scrolling through the projected scroller. Add tests asserting visible movement and delivered offsets.

4. **Must-fix — Agent activation uses the hidden row’s geometry and bypasses occlusion checks.** [AgentIOS.swift:461](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:461), [GroupedListIOS.swift:103](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:103)  
   The initial viewport check measures the authored row, whose scroll remains stationary. After scrolling the collection, a visible lower row can be rejected while an offscreen former first row can still activate. The new early return also bypasses the ordinary keyboard/overlay hit-test checks; having a collection in a window is insufficient. Resolve the actual cell/accessory before checking geometry and availability. Test scrolled and occluded targets.

5. **Must-fix — Hidden sections and rows reappear on iOS.** [kernel/src/grouped.rs:110](/tmp/rv-grouped/kernel/src/grouped.rs:110)  
   The model walks every child without checking display state. A section or standard row authored with `display="none"` remains in the snapshot and receives a visible UIKit cell, although the web hides it. Exclude effectively `display:none` sections/groups/rows from the projection and test visibility changes without removing their nodes.

6. **Must-fix — Arbitrary nested views are mistaken for subtitle columns.** [kernel/src/grouped.rs:196](/tmp/rv-grouped/kernel/src/grouped.rs:196)  
   `row`, `column`, and `box` all lower to `NodeType::View`. Consequently, a row containing a nested horizontal `row` with two texts becomes a native subtitle cell, losing its horizontal layout and authored styling. Contract only applies subtitle styling to `column`. Make the kernel’s recognition agree with that shape; other nested boxes must remain custom. Add horizontal-row and ordinary-box counterexamples.

7. **Must-fix — The sheet overrides author classes on descendants.** [contract/lower/src/grouped.rs:466](/tmp/rv-grouped/contract/lower/src/grouped.rs:466)  
   Generated attributes are inserted before descendant class expansion. `class_rows()` treats these as explicit attributes and discards matching class declarations. For example, a row class specifying `min-height=80` loses to the generated `52`; header and part classes have the same problem. Expand descendant classes before adding the sheet, preserving sheet → class → explicit-attribute precedence. The existing override test covers only explicit attributes.

8. **Must-fix — Conditional leading symbols receive incompatible spacing.** [contract/lower/src/grouped.rs:429](/tmp/rv-grouped/contract/lower/src/grouped.rs:429), [contract/lower/src/grouped.rs:496](/tmp/rv-grouped/contract/lower/src/grouped.rs:496)  
   With `when showIcon` as the first child, `icon` is false, so the row receives a 16-point margin. Yet `part()` descends through the condition and gives the symbol `margin-left=-40`, moving it outside the clipped group. UIKit recognizes the live symbol correctly. Compute the inset and part positions consistently across control-flow branches; test both states. `match` and `each` parts also currently bypass this styling traversal.

9. **Should-fix — Restoring multiple custom rows can reorder siblings.** [GroupedListIOS.swift:336](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:336), [GroupedListIOS.swift:349](/tmp/rv-grouped/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:349)  
   Saved indices reflect siblings already removed by earlier carries, and restoration iterates an unordered dictionary. From `[A, B, C]`, carrying custom A then C records indices 0 and 1; restoring A then C produces `[A, C, B]`. Preserve authored sibling order independently of removal order. Extend the single-custom-row test to multiple custom rows interleaved with standard rows.

10. **Should-fix — Header/footer contents are never validated.** [contract/lower/src/grouped.rs:259](/tmp/rv-grouped/contract/lower/src/grouped.rs:259)  
    The compiler checks position and multiplicity but accepts arbitrary label children. A footer containing two texts or an image compiles; the kernel takes only the first direct text and iOS silently drops the remainder. Enforce D3’s supported text shape and add refusal tests.

11. **Should-fix — Plain lists still receive grouped section gaps.** [contract/lower/src/grouped.rs:251](/tmp/rv-grouped/contract/lower/src/grouped.rs:251)  
    `plain` avoids `FIRST_GAP` but still receives `SECTION_GAP` above headerless sections and below footerless sections. Two plain sections therefore have unwanted spacing, contrary to LLP 1082’s no-gap metrics. Set plain section margins to zero and test actual layout, rather than only the model’s style string.

Validation: caps passed. Cargo tests could not start because the sandbox denied build-directory creation. No files were changed; no servers or simulators were started.

Verdict: DO NOT LAND
