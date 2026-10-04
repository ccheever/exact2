# Code review: LLP 1084, the native grouped list (origin/main..cc5d28641), 2026-10-03 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `cc5d28641`.
- **Method:** one brief (sha256 `542ffb5020084e4a6ebf1434d1bde925b6d1a116ce6a5c9735b10254487b2147`), the same one sent to grok; round 1; blind to the other review. Requested by the coordinator for Charlie. The author (Claude) is not a reviewer.
- **Renumbered:** LLP 1084 was LLP 1082 while it was reviewed (main had two 1082s). The transcripts below keep the old number and paths.
- **Transcription:** the run's final message (`--output-last-message`), unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** each finding checked in the source. Ten are fixed in `25a5b053c`; #3 is fixed for the agent and argued for the rest.
  1. *Cached switch target.* Fixed. The switch's action reads the row's current target when it fires (`flip(id, …)`). Tested in `testASwitchFollowsItsControlsStateAndTarget`, where a replaced control is the one flipped.
  2. *Accessory disabled state.* Fixed. `refresh` runs on every `mount` and sets the switch's `isEnabled` from the row and its current control. `detail(_:)` checks the current button's `disabled` and `inert`. Tested: a control disabled and then re-enabled with the row unchanged, and the agent's tap refused while it is disabled.
  3. *Scroll plumbing.* The agent's wheel now scrolls the collection view (`GroupedCollectionView`, `Agent.scroll`), driven on the simulator: rows below the fold were then tapped. Authored `scrollTop` writes and `scroll` events are argued and declared (LLP 1084 §4). The kernel's content height is the sheet's, not UIKit's, so mirroring one offset onto the other is wrong at both ends. A settings screen needs neither; a consumer that does gets it designed then.
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


## Round 2, 2026-10-03

- **Method:** `codex exec` as round 1, `-C` a detached worktree at `41bff3539`; brief sha256 `05b80dea1c2a65ea9266c8daec3b2fb3326b12b6bf2883f1c09a1a9ce6a58a46` (round 1's brief, plus: check the fixes `cc5d28641..41bff3539` and the dispositions, then review the whole again). Blind to grok's round 2.
- **Verdict:** DO NOT LAND.
- **Disposition:** all fixed in `b8664e6bf` except #8, which is declared:
  1. *Inert.* Fixed. Highlight, selection and the agent's tap check the row node's `inert`, which walks its ancestors, so an inert section counts. An inert row's cell takes no touch and its accessibility elements are hidden, refreshed on every mount. Tested across batches.
  2. *The keyboard.* Fixed. `activate` refuses a cell whose middle is under the keyboard guide, as `Agent.obscured` does.
  3. *Custom rows under the agent.* Fixed. A custom row is not a row the host draws: the ordinary tap path hit-tests its carried views, so a nested button is the one pressed. Tested (`draws` is false for a custom row).
  4. *A text after a condition.* Fixed. The count is symbolic: where a `when`'s arms write different numbers of texts, the next text's `flex-grow` and colour are choices on its condition. Tested in both states. A `match` counts the fewer texts and is declared (it has no condition to choose by).
  5. *The subtitle predicate.* Fixed. `text_stack` uses the kernel's accessory rules (symbol, `input`, an info-only button) and reads a `when` at either end. Tested with a conditional checkmark and with a text button.
  6. *A leading `match`.* Fixed. A symbol in first place inside a `match` is not styled as leading, which matches its inset.
  7. *Native button rows.* Fixed. They get only the rows a native button takes, and the kernel reads them as custom. Tested: it compiles and is custom.
  8. *First section under control flow.* Declared (LLP 1084 §4): only a section written first gets the 35.33-pt gap.
  9. *Lifecycle coverage.* Added: a row turning custom and back, with its views carried and given back, and a header's text changing.

---

The switch target/state, class precedence, hidden-row filtering, label validation, plain gaps, and reverse restoration fixes look correct by inspection. I accept the documented scroll-write/event deferral and the decision that custom rows retain their own touch handling. Several fixes remain incomplete.

1. **Must-fix — Native cells bypass `inert`.** [GroupedListIOS.swift:417](/tmp/rv-grouped2/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:417)  
   `select` and `shouldHighlightItemAt` check only `disabled`. A standard `button inert=true press=go` still highlights and dispatches `go`: its inert NodeView is hidden behind an independent cell. An inert section has the same problem. Preserve effective authored inertness in the projection and enforce it for selection, interaction, and accessibility. Test changing row and section inertness across batches.

2. **Must-fix — Keyboard occlusion is still bypassed.** [GroupedListIOS.swift:116](/tmp/rv-grouped2/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:116)  
   The new window hit-test handles overlays in the app window, but the software keyboard requires the separate check in [AgentIOS.swift:586](/tmp/rv-grouped2/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:586). Grouped activation returns before that check. A cell beneath an overlapping keyboard can therefore activate and change state. Apply the keyboard-guide check using the projected cell’s position; test refusal without dispatch. Round 1’s occlusion fix is incomplete.

3. **Must-fix — Agent taps on custom rows bypass their retained touch handling.** [GroupedListIOS.swift:132](/tmp/rv-grouped2/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:132)  
   `list(drawing:)` includes custom rows, and activation always selects the outer row. For a custom button containing another button at its middle, a finger presses the inner button; the agent presses the outer button instead. This contradicts the accepted custom-row disposition. Let custom rows use the ordinary hit-target activation path through their carried views. Add a nested-control test.

4. **Must-fix — Conditional text styling now fails in the opposite branch.** [contract/lower/src/grouped.rs:588](/tmp/rv-grouped2/contract/lower/src/grouped.rs:588)  
   For `when dark → text "New"` followed by `text "Notifications"`, taking the minimum branch count permanently styles “Notifications” as a title. With `dark=true`, the kernel correctly reads it as the secondary value, but the sheet gives both texts title styling and `flex-grow=1`. The new test checks only `false`. Make subsequent styling conditional on the actual preceding text count and test both states, including `match`.

5. **Must-fix — The new subtitle predicate still disagrees with the kernel.** [contract/lower/src/grouped.rs:666](/tmp/rv-grouped2/contract/lower/src/grouped.rs:666)  
   A two-text `column` followed by a conditional checkmark fails `text_stack`, losing subtitle padding, secondary colour, and sizing. The kernel recognizes a subtitle cell in both states. Conversely, this predicate strips *any* trailing button/input, so a column beside an ordinary text button gets subtitle styling even though the kernel considers the row custom. Match the kernel’s accessory rules and handle conditional parts; test both counterexamples.

6. **Must-fix — Leading symbols under `match` are clipped away.** [contract/lower/src/grouped.rs:480](/tmp/rv-grouped2/contract/lower/src/grouped.rs:480)  
   `part` now descends into `match` and applies the leading symbol’s `margin-left=-40`, but `inset` handles only `when`, leaving the row inset at 16. The symbol falls outside the clipped group while UIKit displays it normally. The documented “styled as no symbol” limitation does not justify this incompatible pair of styles. Compute the inset consistently with the branch traversal and test both arms.

7. **Should-fix — Generated sheet rows make native-button rows fail compilation.** [contract/lower/src/grouped.rs:501](/tmp/rv-grouped2/contract/lower/src/grouped.rs:501)  
   A section containing `button appearance="auto" press=go` receives generated `flex-direction`, padding, border, and text styles. Native-button validation then rejects these as forbidden authored styles, although the author supplied none. Preserve native controls through the custom-row escape hatch, applying row decoration without injecting incompatible control styles. Add a compilation and projection test.

8. **Should-fix — “First section” is still computed from syntax position.** [contract/lower/src/grouped.rs:153](/tmp/rv-grouped2/contract/lower/src/grouped.rs:153)  
   If the first `when` contains two headerless sections, both receive the 35.33-point first-section gap. If that `when` is empty, the following section receives only 17.33 despite becoming first. An initial `each` still never receives the first gap. Determine firstness across the rendered sequence, or implement equivalent list-level spacing. The claimed `when`/`match` fix is incomplete; test multiple and empty branches.

9. **Should-fix — UIKit lifecycle coverage remains too narrow.** [GroupedListIOSTests.swift:196](/tmp/rv-grouped2/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:196)  
   The batch test adds a checkmark and removes the list. It does not exercise actual cell reuse, a surviving row changing between standard and custom, or supplementary insertion/removal during section reordering. Add those cases, checking carried-view ownership, restored hierarchy, cell heights, and current header/footer text.

Validation: caps passed. Cargo tests could not start because the sandbox denied build-directory creation. No files were changed; no servers or simulators were started.

Verdict: DO NOT LAND


## Round 3 (the last), 2026-10-03

- **Method:** `codex exec` as before, `-C` a detached worktree at `79bc59572`; brief sha256 `c122581fb838da00d43c1993e71b6e883fcdc98ed7f0686675718601c5a0557a` (check round 2's fixes and declarations, then the whole once more; only what matters for landing). Blind to grok's round 3.
- **Verdict:** DO NOT LAND.
- **Disposition:** all six fixed in `57df99052` (rebased; `b8664e6bf` is `b266ae6d3` after the rebase onto `8098e9d42`), each with a test. The author lands after this round, as the rules cap review at three rounds.
  1. *A carried custom row's inertness.* Fixed. A row's inertness is recorded from its authored ancestors when it is carried. The cell's interaction and accessibility, and the row's press, use that record. Tested with an inert section.
  2. *A class-made native button row.* Fixed. After class expansion the lowering finds the effective `appearance`, keeps only the rows a native button takes, and strips the sheet from its face (`grouped::native_rows`, `grouped::unsheet`). Tested: it compiles.
  3. *A wheel on a custom row its cell's reuse took off screen.* Fixed. `scroller(for:)` walks up from the node to a row a list's model names, custom rows included, and the wheel is handled before the on-screen guard. Tested.
  4. *A conditional first subtitle line.* Fixed. The column's lines are counted as a row's texts are, so the next line's size and colour are choices on the condition. A hidden text is not counted. Tested in both states.
  5. *Any `input` as an accessory.* Fixed. Only `input type="checkbox"`, the kernel's toggle, counts. Tested with a text field.
  6. *A hidden leading symbol.* Fixed. The inset, the leading position and the stack all skip a literal `display="none"` part. Tested.

---

1. **Must-fix — Carried custom rows lose their section’s `inert` state.** [GroupedListIOS.swift:306](/tmp/rv-grouped3/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:306)  
   `mount()` calls `carry()` before `interact()`. Carrying removes the section from the row’s UIKit ancestry, which `NodeView.inert` walks. A custom button inside `section inert=true` therefore becomes interactive and accessible again; its ordinary activation path also misses that ancestor. Preserve effective authored inertness across reparenting, including nested controls. Extend the inert test to custom rows and section-state changes.

2. **Must-fix — Native-button rows still fail when appearance comes from a class.** [grouped.rs:463](/tmp/rv-grouped3/contract/lower/src/grouped.rs:463)  
   The native-button exception examines unexpanded attributes. With `style Native` containing `appearance="auto"`, a `button class=Native` receives the ordinary sheet. Subsequent class expansion makes it native, and native-button validation rejects generated `flex-direction`, padding, and border styles. Resolve effective appearance after classes before choosing the sheet. Test the class-based form alongside the existing explicit-attribute test.

3. **Should-fix — Cell reuse breaks wheel routing for custom rows.** [GroupedListIOS.swift:330](/tmp/rv-grouped3/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:330), [AgentIOS.swift:478](/tmp/rv-grouped3/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:478)  
   When a custom row’s cell is reused, its NodeView is detached until the next batch restores it. A wheel targeting that row then fails the `v.window` guard. Moreover, `scroller(for:)` excludes custom rows from model lookup and cannot find a detached row through ancestry. Resolve grouped wheel ownership before requiring a window, using model/carry bookkeeping. Add an actual scroll-and-reuse test; the current wheel test uses a standard row.

4. **Should-fix — A conditional first subtitle line permanently demotes the following title.** [grouped.rs:820](/tmp/rv-grouped3/contract/lower/src/grouped.rs:820)  
   For a column containing `when show → text "New"` followed by `text "Notifications"`, `*first = *first && after` styles “Notifications” as a subtitle in both states. When `show=false`, the kernel correctly makes it the title, but the sheet renders it at 15 points in secondary colour. A hidden first text has the same problem. Track the visible text position conditionally and test both states and hidden text inside subtitle columns.

5. **Should-fix — The subtitle predicate still accepts every input as an accessory.** [grouped.rs:755](/tmp/rv-grouped3/contract/lower/src/grouped.rs:755)  
   `"input" => true` remains despite the round 2 disposition. A two-text column beside `input type="text"` or a range input receives subtitle padding and typography, although the kernel recognizes only checkbox/switch accessories and makes this row custom. Restrict the predicate to the same control kinds as the kernel. Add text-field and range counterexamples.

6. **Should-fix — Hidden leading symbols still reserve the symbol inset.** [grouped.rs:509](/tmp/rv-grouped3/contract/lower/src/grouped.rs:509)  
   A row beginning with `image "symbol:person" display="none"` followed by its title receives the 56-point inset. The kernel excludes the hidden image and UIKit displays a title-only cell with the ordinary inset. Skip statically hidden parts when determining the leading symbol, consistently with text counting. Test the resulting title position.

Caps passed. Cargo tests could not start because the sandbox denied build-directory creation. No files were changed; no servers or simulators were started.

Verdict: DO NOT LAND
