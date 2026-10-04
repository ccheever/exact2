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


## Round 2, 2026-10-03

- **Method:** grok as round 1, `--cwd` a detached worktree at `41bff3539`; brief sha256 `05b80dea1c2a65ea9266c8daec3b2fb3326b12b6bf2883f1c09a1a9ce6a58a46`. Blind to astra's round 2. stdout, unedited; the first paragraph sentences are progress narration.
- **Verdict:** DO NOT LAND.
- **Disposition:** fixed in `b8664e6bf` unless noted:
  1. *A text after a `when`.* Fixed: the next text's style is a choice on the condition (as astra #4), and the true state is asserted. `match` is declared.
  2. *`text_stack` through `when`.* Fixed: a `when` showing the second line counts, the second line is styled inside its arm, and a leading `when` symbol is read through. Tested.
  3. *A leading `match`.* Fixed (as astra #6).
  4. *First-section gaps.* Declared, by the simpler rule: only a section written first gets 35.33. The list-padding scheme would be wrong when the first rendered section has a header.
  5. *The wheel on a row below the fold.* Fixed. `scroller(for:)` resolves the list for a row it draws, a carried view or the list itself, and the wheel scrolls that. Tested: the collection moves and the hidden scroll does not.
  6. *A hidden text counted.* Fixed: a text written `display="none"` is not counted. Tested.
  7. *A switch rebuilt inside its own action.* Fixed. The firing row is left out of that batch's reconfigure and reconfigured after the action returns. Tested.
  8. *Any trailing button or input.* Fixed (as astra #5).

---

I'll review the grouped-list change from the merge-base through HEAD, starting with the LLP, the recorded dispositions, and the diff since round 1.The branch is three commits past `origin/main`: the LLP 1082 implementation, the round-1 fixes, and the recorded dispositions. I'll read those dispositions and the spec before checking the fixes.Round 1’s dispositions are in. Next I’ll read the sheet, the kernel reader, and the iOS host to check each fix and look for anything the first pass missed.The round-2 review is the remaining deliverable. I'll confirm the line numbers on the fixes and the two host splits before writing it.Round 2 of LLP 1082. The UIKit fixes in `25a5b053c` hold: class rows beat the sheet, the leading-symbol inset is a real ternary on `when` (both states tested), hidden sections and whole rows drop out of the kernel model, a horizontal `row` of two texts stays custom, plain sections meet, a header or footer must be one `text`, custom rows restore last-carried first, the switch reads its target when it fires and is taken out of its superview before the accessory is built, `disabled` and `inert` refresh on mount, layout invalidation happens after `apply`, and a tap resolves the cell (off-port and covered middles refused, and that check runs before the control host). `cargo test -p contract --test it grouped_list` passes, 10 tests. The Swift suite was not run.

Two arguments hold. A custom row takes no cell highlight (§4): the carried views keep their press, press feedback, and nested controls, and a standard row still highlights. Authored `scrollTop` and `scroll` events stay on the hidden scroll (§4): the kernel's content height is the sheet's, so one offset mapped onto the other is wrong at both ends. `groupedList.offset` is the visible one. `layout`'s `sx`/`sy` still reads the hidden scroll (`AgentIOS.swift:228`).

Two claims do not hold, and they are findings 4 and 5. The sheet still disagrees with the kernel on two shapes D4 reads live. That is enough to keep this from landing.

1. **Must-fix — A text after a `when` or `match` is styled for the arm that wrote fewer texts.** `contract/lower/src/grouped.rs:588` and `:608`, applied at `:647`.

`part` styles each arm from the same start count, then sets the counter to the minimum of the two. The following text is therefore the title whenever either arm is empty. The kernel reads the live arm.

```
button
  when unread
    text "New"
  text "Notifications"
```

With `unread` false the kernel has one title, and the web gives "Notifications" `flex-grow: 1`. That is the only state `a_conditional_text_and_an_authors_column_are_styled_as_the_kernel_reads_them` locks (`contract/cli/tests/it/grouped_list.rs:306`). The disposition says the same thing and calls it fixed. With `unread` true the kernel has a value cell (title "New", secondary "Notifications") and iOS paints `.valueCell()`. The web gives both texts `flex-grow: 1` and the primary color, because the counter was put back to 0. A text before the `when` is fine: it is always text 1, and a text inside the arm is styled at the count that includes it. The same `min` is on `match`.

Fix: give the following text a ternary `flex-grow` and color on that condition, the way `inset` (`:480`) is a ternary for a leading `when`. Assert the true state. The current assertion cannot fail there, because the style is not conditional.

2. **Must-fix — `text_stack` does not read through `when` or `match`, so a subtitle cell on iOS is an unpadded column on the web.** `contract/lower/src/grouped.rs:666`. The kernel test is `kernel/src/grouped.rs:203`.

`text_stack` strips one leading non-accessory symbol element and requires the column's direct children to all be `text`. D4 (`llp/1082-native-grouped-lists.rfc.md:84`) says a `when` between parts counts, and D7 (`:121`) says a `when` is read through. `when` is not a node in the live tree, so `shown` (`kernel/src/grouped.rs:147`) sees the texts.

```
button
  column
    text "Privacy"
    when extra
      text "Screen lock"
```

The `when` fails `children.iter().all(|c| is(c, "text"))` (`grouped.rs:681`). The sheet applies neither the 15 pt padding nor the 15 pt secondary. In both states the kernel sets `subtitle` (one or two texts). iOS uses `.subtitleCell()` (68 pt, 15 pt secondary). The web shows a 52 pt row of 17 pt primary text.

The same split happens with the symbol in front of the column:

```
button
  when hasIcon
    image "symbol:bell"
  column
    text "Privacy"
    text "Screen lock"
```

The leading child is a `when`, so it is not stripped, `stack` stays false, and with `hasIcon` false the kernel still sees a two-text column. The icon inset ternary itself is correct.

Fix: read through `when` and `match` in `text_stack`, and treat the column as the stack when every arm leaves one or two texts. An optional second line is a subtitle cell in both states, so the static 15 pt padding is right. Style that second text inside the arm. `subtitle` (`:703`) currently skips anything that is not a direct `text`. The existing test covers a custom profile column and a horizontal `row`, which this shape is not.

3. **Should-fix — A leading `match` pulls the symbol out of a row that was styled as having no symbol, and the group clips it.** `contract/lower/src/grouped.rs:480` and `:621`, with the `i == 0` place passed in at `:604`.

D7 (`:119`) styles a `match` or `each` in first place as no symbol. `inset` does that: only a `when` becomes a ternary, so a leading `match` gets `margin-left: 16`. `part` still walks the `match` and, because the place is `i == 0`, gives the image `margin-left: -40`. The group is `overflow: hidden` (`:361`). The symbol is clipped and the title sits at 16. iOS draws a normal symbol cell, because the kernel sees the live image.

```
button
  match mode
    some
      image "symbol:bell"
      text "Title"
```

An `each` in that place stays consistent, because `part` does not walk into it. Keep the trailing-accessory walk so a checkmark inside a last-child `match` stays styled. Do not pass `i == 0` into a leading `match`.

4. **Should-fix — The first-section gap is wrong for a leading `each`, and wrong between sections inside a leading `when` or `match`.** `contract/lower/src/grouped.rs:152` and `:300`.

Headerless sections use `margin-top: 35.33` when `first` is set and `17.33` otherwise (`FIRST_GAP`, `SECTION_GAP`). Sibling margins collapse to the max, and the list's scroll is a new block formatting context, so those numbers are the gaps.

A direct `each` forces `first` false for every section in the shared body. Between headerless sections, `17.33` collapsed with `17.33` is the right `17.33`. The first of those sections is `18` pt short of UIKit's `35.33`. The shared body cannot mark one iteration. A list `padding-top` of `18` when that `each` is the first child and its section has no header yields `18 + 17.33 = 35.33`, because padding blocks collapse. A section with a header already has top margin `0`, so that padding stays off.

`over` (`:173`) applies the same `first` flag to every element it reaches. A leading `when` or `match` is not an `each`, so `first` is true, and every headerless section inside it gets `margin-top: 35.33`. The collapsed gap between them is `35.33` instead of `17.33`. That includes an `each` nested in that `when`:

```
list appearance="auto"
  when ready
    each item in items
      section
        button
          text item.name
```

Pass `first` only to the first section of a first-child `when` or `match`.

5. **Should-fix — A wheel whose target row sits outside the list's port scrolls the hidden scroll.** `host/apple/Sources/ExactKit/IOS/AgentIOS.swift:531` and `:601`.

`scroll(from:)` moves a `GroupedCollectionView` when the walk is already on one. That happens for a wheel on the list node, and for a row whose layout middle hit-tests the collection. `activate` (`GroupedListIOS.swift:109`) resolves the cell instead. The wheel does not. It starts at the hit of the node's layout middle (`AgentIOS.swift:483`). A row below the fold has that middle outside the list. Hit-testing misses the collection, which is a sibling of the hidden scroll, and the walk starts at the hidden row. The hidden `ScrollView` is an ancestor and takes the tick (`:606`). The cells do not move. `layout`'s `sx`/`sy` then reports the hidden offset (`:228`) while `groupedList.offset` stays put. After the collection has scrolled, a row that is visible in a cell still has its kernel box at the unmoved layout position, so wheeling that row id takes the same path. The tap error tells the agent to scroll the row into view first. No test calls `Agent.scroll`. The scrolled-away test sets `contentOffset` on the collection.

Fix: when `draws(id)`, scroll that list's collection view, the same resolution `activate` uses.

6. **Should-fix — A literal `display="none"` part still counts as a text in the sheet.** `contract/lower/src/grouped.rs:647`. The kernel skips it at `kernel/src/grouped.rs:151`. D4 says those children are not counted (`llp/1082-native-grouped-lists.rfc.md:91`).

```
button
  text "Gone" display="none"
  text "Stay"
```

The sheet styles "Gone" as the title (`flex-grow: 1`) and "Stay" as the value (secondary, no `flex-grow`). The kernel and iOS read "Stay" as the title. `hidden_parts_and_a_row_of_texts_are_read_as_the_web_shows_them` hides a whole row and a whole section (`grouped_list.rs:279`), so this path does not fail it. The idiomatic conditional is `when`, which is finding 1. Skip a literal `display="none"` child in the text counter and the inset, and test a part inside a visible row. A bound `display` is not visible at compile time.

7. **Should-fix — A switch flip that changes the row's parts removes the switch from its superview inside its own action.** `host/apple/Sources/ExactKit/Session.swift:675` and `host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:334`.

`onChecked` calls `apply` before returning. `flip` (`:357`) runs from the switch's `.valueChanged`. If that batch changes the row model (a title "On"/"Off", a sibling `when`), `update` reconfigures the item (`:241`) and `accessories` calls `toggle.removeFromSuperview()` on the switch that is firing, then hands it back to `.customView`. A batch that only changes `checked` leaves the row model equal, so it does not reconfigure. That path is the one the test drives, and the replacement-target test applies its batch from outside the action. This path was not run (no simulator). Keep a switch that is already installed in that cell, or defer the reconfigure of the firing cell until the action returns.

8. **Nit — `text_stack` treats any trailing `button` or `input` as the accessory.** `contract/lower/src/grouped.rs:674`.

A two-text column followed by a text field or an ordinary button gets the 15 pt subtitle padding. The kernel marks that row custom, and iOS carries the views, so both hosts show the padding. Narrow the strip to a real accessory (symbol, switch, checkbox, or the info button) if that padding should stay the author's.

Verdict: DO NOT LAND


## Round 3 (the last), 2026-10-03

- **Method:** grok as before, `--cwd` a detached worktree at `79bc59572`; brief sha256 `c122581fb838da00d43c1993e71b6e883fcdc98ed7f0686675718601c5a0557a`. Blind to astra's round 3. stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** fixed in `57df99052` unless noted:
  1. *`display="none"` beyond a direct text.* Fixed: the inset (inside a `when` arm too), the leading position, the line count and the line walk all skip it. Tested with both shapes.
  2. *A `when` hiding a subtitle's first line.* Fixed (as astra #4). Tested in both states.
  3. *A class-made native button row.* Fixed (as astra #2). The snippet compiles, and the kernel reads the row as custom.
  4. *A class's `display: none`.* Declared (LLP 1082 §4). A class is applied when the part is lowered, after the row's parts are counted.

---

I'll review the LLP 1082 change from the merge-base through HEAD, starting with the RFC and the commits since round 2, then check each declared fix against the current code.The branch is five commits past `origin/main`. I'll read the RFC and both round-2 reviews, then check the `b8664e6bf` fixes against the code.Round 2's dispositions are recorded. Next I’ll read the sheet, the kernel reader, and the iOS host and check each claimed fix.Round 3 is a landing check of the round-2 fixes and anything still wrong in the whole change. I'll verify the remaining contract/kernel disagreements against the source and a throwaway compile before writing findings.The round-2 host fixes match the code. I'm compiling a class-styled native button inside a grouped list to confirm whether that path is still refused.Round 3 of LLP 1082, through `79bc59572`. The round-2 fixes in `b8664e6bf` match the source: conditional text styling, subtitle stacks with a second line under `when`, leading `match`, the wheel, the switch reconfigure, inert, the keyboard check, and custom rows on the ordinary tap path. The §4 limits that were declared (first-section gap, `match` counting the fewer texts, `scrollTop` on the hidden scroll, no cell highlight on a custom row) match the code. `cargo test -p contract --test it grouped_list` passed, 13 tests. The Swift suite was not run.

Three sheet holes are still open. None of them are on the fixture’s common path.

1. **Should-fix — A literal `display="none"` is skipped only on a direct `text`.** `contract/lower/src/grouped.rs:509` (`inset`), `:661` (leading-image arm), `:777` (`lines`), `:828` (`subtitle`). `hidden` is consulted only at `:688`. The kernel skips every `display: none` child before it reads a symbol or a column (`kernel/src/grouped.rs:152` and `:212`).

   A hidden leading symbol still takes the icon inset:

   ```
   button
     image "symbol:bell" display="none"
     text "Notifications"
   ```

   `inset` sees a non-accessory symbol as the first child, so the row gets `margin-left: 56`. The image is not shown, so the web title starts in an empty icon slot. The kernel has no symbol, and iOS draws the title at the 16 pt inset. The same inset is used when that image is the first child of a `when` arm.

   A hidden text inside the subtitle column is still counted:

   ```
   button
     column
       text "Gone" display="none"
       text "Privacy"
       text "Screen lock"
   ```

   `lines` counts three texts, `text_stack` is false, and the column gets no 15 pt padding. The kernel drops `Gone`, reads two texts, and sets `subtitle`. iOS uses `.subtitleCell()`. With only `Gone` and `Stay`, the stack is accepted and `Stay` is styled as the 15 pt secondary line, while the kernel’s title is `Stay`.

   Skip a literal `display="none"` child before the icon test, the leading-image styles, the line count, and the title/subtitle walk, and take the inset from the next shown child so a hidden image in front of a real symbol still leaves that symbol in the icon slot. Extend `a_subtitle_shown_by_a_condition_and_a_hidden_text` with both shapes.

2. **Should-fix — A `when` that can hide the first line of a subtitle column styles the following text as the second line in both states.** `contract/lower/src/grouped.rs:820`.

   `text_stack` accepts this (`:787`: low 1, high 2):

   ```
   button
     column
       when extra
         text "Privacy"
       text "Screen lock"
   ```

   `*first = *first && after` becomes false if either arm wrote a text, so `Screen lock` is 15 pt secondary even when `extra` is false. In that state the kernel’s only line is the title `Screen lock`, and iOS draws it as the subtitle cell’s primary title. With `extra` true the two hosts agree. The shape the RFC names — title outside, second line inside the `when` — is styled correctly and is what `a_subtitle_shown_by_a_condition_and_a_hidden_text` locks.

   Give the following text a choice on that condition, the way a text after a `when` already chooses `flex-grow` and color (`:714`): title metrics in the empty arm, 15 pt secondary in the arm that wrote a line. Assert both states.

3. **Should-fix — A native button whose `appearance="auto"` comes from a class is given the full sheet and then refused.** `contract/lower/src/grouped.rs:463`. `native_button` reads the class (`contract/lower/src/controls.rs:382`); `row` looks only at the element’s own attributes, and `sections` runs on the syntax children before those children are lowered (`contract/lower/src/lib.rs:734`).

   ```
   style Platform
     appearance="auto"
   list appearance="auto"
     section
       button class=Platform press=go
         text "Save"
   ```

   `row` emits `flex-direction`, borders, padding, `font-size`, and `color`. After class expansion the button is native, and `check_native_button` rejects the sheet (`controls.rs:634`). Compiling that file failed with `lower-button-style-attr` on `flex-direction`. LLP 1069 already treats this class as a native button (`the_switch_is_a_literal_after_classes`). A button that writes `appearance="auto"` on itself gets only `margin-left: 16` and `min-height: 52`, and that path is tested.

   Choose the sheet from the effective appearance, including class rows, and keep the two-row sheet for that button. The snippet above should compile, and the kernel should report the row custom.

4. **Nit — `hidden` does not see a class.** `contract/lower/src/grouped.rs:730`. A `style Gone` of `display none` is a style row, so it is known when the file compiles, but it is applied only when that text is lowered, after the sheet has counted it. `text class=Gone "Gone"` followed by `text "Stay"` still styles `Gone` as the title and `Stay` as the value. The kernel’s title is `Stay`. Resolve a class’s literal `display` before counting, or this split survives the fix in finding 1.

Verdict: LAND WITH FIXES
