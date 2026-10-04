# Code review: LLP 1082, the native grouped list (origin/main..cc5d28641), 2026-10-03 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, one fresh session with `--cwd` a detached worktree at `cc5d28641`. `--always-approve` because plan mode cancels shell reads. The brief instructs read-only, and the worktree was clean afterwards.
- **Method:** the shared brief (sha256 `542ffb5020084e4a6ebf1434d1bde925b6d1a116ce6a5c9735b10254487b2147`); round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited. The first two sentences are progress narration.
- **Verdict:** DO NOT LAND.
- **Disposition:** each checked in the source. Fixed in `25a5b053c` unless noted:
  1. *Classes under the sheet.* Fixed (the same as astra #7).
  2. *`when` and the icon inset and text order.* Fixed. The inset is a choice on the leading `when`. A text after a `when` counts the fewer of the two arms' texts, so it is styled as the title the kernel reads while the condition is false. Tested.
  3. *Any `View` as a subtitle.* Fixed (the same as astra #6).
  4. *Plain gaps.* Fixed (the same as astra #11).
  5. *Scroll.* The wheel is fixed; `scrollTop` and scroll events are argued (the same as astra #3). `layout`'s `sx`/`sy` for the list reads the hidden scroll; `groupedList.offset` is the visible one, and §4 says so.
  6. *The switch's target, and a custom view that keeps its superview.* Fixed. The target is read when the switch fires, and the switch is removed from its superview before each accessory is built.
  7. *Invalidating layout inside the data source's update.* Fixed. `carry` only records a height change, and `mount` invalidates once after `apply`.
  8. *Every `column` styled as a subtitle.* Fixed. Only the row's text stack (the shape the kernel reads, `text_stack`) gets the subtitle padding; the fixture's profile card no longer does. Tested.
  9. *The first gap under control flow.* Fixed for `when` and `match` in first place. Under `each` every section shares one body, so none is first, as the comment says.
  10. *One-text labels.* Fixed (the same as astra #10).
  11. *A custom pressable row takes no cell highlight.* Argued and declared. The carried views keep their own touch handling: a custom row's press, press feedback and nested controls stay the author's, so the escape hatch carries no surprises. LLP 1082 §4 says so.
  12. *The agent's tap on a disabled row's toggle.* Fixed by astra #2 and #4: the grouped check runs before the control host's and refuses a disabled switch.
  13. *`match` in `part`.* Fixed.
  14. *Tests.* Added: class precedence, a conditional symbol, a conditional text, plain sections, a `row` of texts, the custom column's padding, hidden parts, one-text labels; on UIKit, a switch's target and enabled state across batches, a scrolled-away tap, three custom rows' order, and the agent's tap on a toggle.

---

I'll review the LLP 1082 grouped-list change only: read the RFC and repo rules, then inspect the diff from `origin/main` to `HEAD` for correctness, design, and test gaps.The change is one commit. I'll read the Contract sheet, kernel reader, and iOS host next, then check them against the RFC and each other.The grouped-list commit is structurally in the right place: components are inlined before the shape check, author attributes do override the sheet, and a direct trailing `when` around a checkmark matches the kernel. Several paths the RFC calls out do not.

1. **Must-fix** — A class cannot replace the sheet. `contract/lower/src/grouped.rs:466` (same prepend in `section` at 257, header/footer at 283, `with` at 383, `subtitle` at 599) and `contract/lower/src/class.rs:21`.

The sheet is written onto the syntax node before lowering. `class_rows` treats any name already on the node as the node's own and drops that row from the class. Last-wins then keeps the sheet ahead of the class. A direct attribute still wins, which is all `the_sheet_draws_ios_metrics_and_the_author_replaces_it` checks.

Failing case: `style Tall` / `min-height 60` / `color #ff0000`, then `button class=Tall` / `text "Delete"`. The row stays 52pt and label-colored. D7 says a class replaces the sheet.

Fix: prepend the sheet in `node()` after `class_rows`, in the same order the list itself uses (`list_rows`, then class rows, then the author's attributes).

2. **Must-fix** — `when` is not read through for the row's icon margin or for text order. `contract/lower/src/grouped.rs:429` and `:495`.

The icon inset looks only at `children.first()` as an element. A leading `when` makes `icon` false, so the row gets `margin-left: 16`, while `part` still gives the image `margin-left: -40`. The group is `overflow: hidden`, so the symbol is clipped away and the title sits at 16pt. UIKit still draws a normal symbol cell because the kernel sees the live image.

```
button
  when hasIcon
    image "symbol:bell"
  text "Notifications"
```

Separately, the text counter keeps the max of both `when` branches. A text after the `when` is styled as the value even when the branch is empty:

```
button
  when unread
    text "New"
  text "Notifications"
```

With `unread` false the kernel has one title. The web paints "Notifications" in the secondary color, without `flex-grow`. iOS paints a primary title.

Fix: derive the row's leading inset from the same walk as `part`, and count texts per branch so a later sibling is styled against each branch's own count (a ternary margin, or styling the title inside the branch).

3. **Must-fix** — The kernel's subtitle test is any `View`, and the sheet's is the tag `column`. `kernel/src/grouped.rs:196`.

D4 says a column of one or two texts is the subtitle cell. `NodeType::View` is also `row`, `view`, `header`, and `section`. A horizontal `row` of two texts becomes `.subtitleCell()` on iOS (title over subtitle, authored layout hidden). On the web `part` leaves that `row` alone, so the texts sit side by side.

Fix: accept the stack only when it is a column (`display: flex` and `flex-direction: column`). Anything else stays custom, so iOS carries the same views the sheet drew.

4. **Must-fix** — A plain list still gets grouped section gaps. `contract/lower/src/grouped.rs:251`.

`style != "plain"` gates only the 35.33 first gap. Every section without a header still gets `margin-top: 17.33`, and every section without a footer gets `margin-bottom: 17.33`. §2 says plain has no gaps. The fixture's last section is only the delete button, so Plain on the web, macOS, and Linux opens an extra 17.33pt above it. iOS plain does not, because UIKit lays that list out.

Fix: for `plain`, use 0 for both section margins.

5. **Must-fix** — The visible list is not the scroll the agent and `scrollTop` drive. `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:237` and `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:607`.

`mount` copies inset behavior and insets, and never `contentOffset`. `Agent.scroll` only moves a `ScrollView`. `UICollectionView` is not one, and the hidden scroll is a sibling of the collection view, so a wheel on a row does not move the cells. `applyPendingScroll` (`NodeViewIOS.swift:910`) writes `scrollTop` to that hidden scroll. `layout` `sx`/`sy` (`AgentIOS.swift:228`) reads it back, so it stays 0 while `groupedList.offset` is the real offset.

Failing case: a row whose layout y is below the viewport. `tap` answers "scroll it into view first". The wheel does not move the collection view, so the row stays untappable. A finger can scroll it.

Fix: point wheel, `scrollTop`, scroll events, and `sx`/`sy` at the collection view, and keep its offset aligned with the authored scroll across batches.

6. **Must-fix** — The toggle's `UISwitch` is created once and then reused illegally. `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:301`.

The action captures `target` when the switch is first built. A later `when` that swaps which checkbox is the accessory leaves the same switch in `switches[row.view]`. `reconfigureItems` calls `accessories` again, but the old action still calls `checked` on the destroyed control, and `flip` returns immediately because that view is gone. The new control never moves.

The same call passes that switch to `.customView` while it still has a superview. `UICellAccessory.CustomViewConfiguration` requires a view with no superview. The second batch that reconfigures a visible toggle (label change, `destructive`, disabled) hits that.

Fix: read `rows[row.view]?.target` inside the action, and `removeFromSuperview()` before building the accessory. Recreate the switch when the target id changes.

7. **Must-fix** — `carry` invalidates layout from inside cell configuration. `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:342`.

`configure` runs while `source.apply` is updating items. `carry` then calls `invalidateLayout()` whenever the custom row's height differs. Invalidating layout during an update is the inconsistency UIKit raises ("Invalidating collection view layout while the collection view is updating items"), and it can re-enter `preferredLayoutAttributesFitting`. The height is already returned from `preferredLayoutAttributesFitting`; the extra invalidation is what makes the pass re-entrant. The XCTest only checks the resting frame after `layoutIfNeeded`.

Fix: set `cell.height` in `configure` and invalidate once from `mount`, after `apply` has returned.

8. **Should-fix** — Every `column` is styled as a subtitle stack, including a custom row. `contract/lower/src/grouped.rs:553`.

`part` calls `subtitle` for any column. The fixture's profile card is a custom row: avatar `row`, a `column` of the name and phone, then the chevron. The kernel correctly marks it custom and iOS carries the views, but those views include the sheet's `padding-top/bottom: 15`, which the card never set. The name drops 15pt relative to the avatar on every host.

Fix: apply subtitle padding only when that column is the row's text stack (the same shape the kernel accepts). Leave any other column to the author.

9. **Should-fix** — The first-section gap is wrong when the first child is `when`, `each`, or `match`. `contract/lower/src/grouped.rs:126`.

`first` is true only when child 0 is an element. D3 allows the sections themselves to sit under control flow. An `each` of header-less sections gives every section `margin-top: 17.33`, including the one UIKit opens at 35.33. The each body is shared, so the extra 18pt has to be padding on the list when that each is the first child and its section has no header.

10. **Should-fix** — A header or footer is not required to be one direct `text`, and iOS then drops what the web shows. `contract/lower/src/grouped.rs:259`, `kernel/src/grouped.rs:116`.

The kernel's label is the first direct text child. A header whose text sits in a `row`, or a header with a symbol plus a text, lowers successfully. The web shows the whole header. iOS sets `header` to that one string, or to nothing, and the other views stay in the hidden scroll.

Fix: refuse a header or footer that is not a single direct non-empty `text` (`lower-grouped-list`).

11. **Should-fix** — A custom button never takes the cell's highlight. `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1377` and `GroupedListIOS.swift:378`.

The fixture's profile card is a pressable custom row. Once carried, the `NodeView` handles `touchesBegan` and does not call super, so the collection view does not run `shouldHighlight` / `didSelect`. The action still fires from `NodeView.touchesEnded`. There is no touch-down highlight, which is the behavior D5 assigns to a pressable row. The XCTest taps a standard cell only.

Fix: for a custom row whose only press is its own, let the cell take the touch (carried views not user-interactive except nested controls) and press from `didSelect`, which already highlights.

12. **Should-fix** — Agent `tap` on a toggle ignores the row's `disabled`. `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:448`.

`controls.activate` runs first and flips the hidden control whenever that control is enabled. The visible `UISwitch` is disabled when the row is (`GroupedListIOS.swift:313`). A finger cannot flip a `row disabled=true` switch. The agent can. D5 says `disabled` makes the row untappable.

Fix: if `list(drawing:)` finds a disabled row whose target is this control, return the grouped-list error before the control host.

13. **Nit** — `part` unwraps `when` and clones `match`. `contract/lower/src/grouped.rs:488`.

A checkmark inside `match` / `some` is a checkmark accessory in the kernel and an unstyled image on the web. D7 names `when`. The same walk should open `match` arms.

14. **Should-fix** — The risky paths have no test. `contract/cli/tests/it/grouped_list.rs`, `host/apple/tests/ExactKitTests/GroupedListIOSTests.swift`.

Covered today: a static settings tree, a trailing `when` checkmark, one author `min-height`, refusals, and on UIKit a single configure, one tap, one toggle snap-back, one carry/restore, one checkmark insert. Missing, each tied to a bug above: a class overriding `min-height` and `color`; a leading symbol inside `when`; a conditional text before the title; `listStyle="plain"` section y; a `row` of two texts staying custom; the profile column's padding; a second batch that changes a toggle's target and its title while the cell is visible; wheel / `scrollTop` moving the collection view; a disabled row's switch under `activate`.

Components are fine: uses are inlined before `grouped::sections`, so a component whose view is a `section` is checked as a section.

Verdict: DO NOT LAND
