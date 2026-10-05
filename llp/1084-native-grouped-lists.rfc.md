# LLP 1084: A grouped list is UIKit's own list on iOS — `list appearance="auto"`

**Type:** RFC
**Status:** Implemented 2026-10-03 (§6, as built). Numbered 1082 until 2026-10-04, when it was renumbered because LLP 1082 (hosting) had landed under the same number. Charlie approved the direction on 2026-10-03: "A native grouped-list role mapped to UICollectionView list layout." That approval is the human word for the fixture page (`scripts/fixtures/grouped-list.contract`) and for the DEFERRED entry this adds (§5).
**Systems:** Contract (`contract/lower/src/grouped.rs`, new: the shape checks and the sheet; `lib.rs`: the hook; `tags.rs`: `listStyle`), Kernel (`schema.json`: prop 225 `listStyle`, the `info` symbol role; `kernel/src/grouped.rs`, new: `Kernel::grouped_list`), Linux host (`paint.rs`, `paint/svg.rs`: symbol roles painted, §4), Apple host (`abi/commands.rs`, `abi/exports.rs`, `include/exact.h`: `exact_grouped_list`; `IOS/GroupedListIOS.swift`, new; `PresenterIOS.swift`, `Bridge.swift`, `Session.swift`, `ChromeIndex.swift`, `AgentIOS.swift`, `AgentNativeIOS.swift`: the wiring), web and macOS (nothing: they draw the sheet)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5), on `grouped-list`, 2026-10-03
**Date:** 2026-10-03
**Related:** LLP 1069.011 (native buttons: `appearance="auto"` as the switch, the children as data, a style vocabulary); LLP 1059 (a tablist projects to a tab bar by shape); LLP 1008 §9 (swipe cells: authored content carried into a UIKit cell, the kernel owning its size); LLP 1010 and LLP 1070 (collections: the virtualized `list`, which this is not); LLP 1080.001 D3 (what the inspection walk accounts for); LLP 1081 D5 (a Contract attribute is outside the `-exact-` rule). The Signal Clone's diary (`~/.tuft/projects/signal-exact2/DIARY.md`, "inset-grouped tables are four small components"): the consumer. Reviews: `llp/reviews/code-2026-10-03-grouped-list.{astra,grok}.md`.

## Summary

Settings screens are inset-grouped lists. Today an app draws one out of boxes: the Signal Clone's `Section`, `Row`, `ActionRow`. The result looks close to iOS, but it is not iOS. There is no cell highlight on touch-down, no dynamic type, no native separators, and VoiceOver reads the rows as buttons.

This RFC makes a `list` whose `appearance` is the literal `auto` a grouped list:

```
list appearance="auto" listStyle="inset-grouped" flex=1
  section
    header
      text "Account"
    button press=openProfile
      image "symbol:person"
      text "Profile"
      image "symbol:forward-chevron"
    row
      text "Read Receipts"
      input type="checkbox" switch checked=receipts input=setReceipts
    footer
      text "Who can see you."
```

On iOS the list is a `UICollectionView` with `UICollectionLayoutListConfiguration`. Its rows are `UICollectionViewListCell`s configured with `UIListContentConfiguration` and cell accessories, so UIKit draws the separators, the highlight, the fonts at the reader's text size and the dark appearance. On the web, macOS and Linux, Contract writes a sheet of ordinary style rows. It is measured from UIKit on iOS 27, and the author's own rows replace any of it. A row Contract cannot read as a standard cell keeps its own views. iOS carries them into their cell; that is the escape hatch.

## 1. What exists

- **`list`** is `NodeType::List` with the role `list`. It scrolls by default (`node.rs:142`). With `virtualized=true` it is a collection (LLP 1010, 1070): the runner builds rows near the viewport. Without it, every row is built and laid out, as in any scroll.
- **Native projections** take three forms, chosen by the shape of the authored tree:
  - A tablist is a segmented control or a tab bar (LLP 1059, by shape).
  - A native button's children are its face, read as kernel data (LLP 1069.011 D5).
  - A swipe row's content is carried into a one-row `UITableView` while a swipe can start (LLP 1008 §9). It goes back before every batch, so the presenter only ever sees the authored hierarchy.
- **Nothing in the Apple host is a `UICollectionView` or a `UITableView` list today.** The collection infrastructure (`Collection.swift`, `CollectionIOS.swift`) is a virtualized window over one `UIScrollView`, laid out by the kernel. Its reason to exist is building rows near the viewport. A grouped list has no such window, so §3 D6 reuses none of it.

## 2. Measured

These numbers come from a scratch UIKit app on the iOS 27 simulator (iPhone 18 Pro, 402 pt wide, 3×). It is a `UICollectionView` with `UICollectionLayoutListConfiguration` in each appearance, with a header and footer per section. The cells are `.cell()`, `.valueCell()` and `.subtitleCell()`, with a disclosure, a checkmark, a detail button and a `UISwitch`. Its subviews were dumped and its pixels read.

| Fact | `.insetGrouped` (light / dark) |
|---|---|
| Background | `#f2f2f7` / `#000000` |
| Cell | `#ffffff` / `#1c1c1e`, 16 pt from each edge, corners radius 26 (circular within 0.5 pt) |
| Row height | 52; subtitle cell 68.33 (title at +15, subtitle 15 pt at +35.33) |
| Symbol | centred at 28 pt from the cell's leading edge, about 20 pt tall, the accent `#0088ff` / `#0091ff` |
| Text | 17 pt regular; at 16 pt from the cell, or 56 pt after a symbol |
| Value | 17 pt, secondary label (`#3c3c43` at 0.6 / `#ebebf5` at 0.6), 8 pt before the accessory |
| Separator | 1 pt, `#3c3c43` at 0.12 / `#545458` at 0.5, from the text to the cell's trailing edge; none under a section's last row |
| Disclosure | 10.33 × 14, tertiary label (0.3), 16 pt from the trailing edge |
| Header | 17 pt semibold, secondary label, 16 pt into the section, 10 above and below (40.33 tall for one line) |
| Footer | 13 pt regular, secondary label, 8 above and 6 below |
| Gaps | footer → next header 0; footer → a section without a header 17.67; a section without a footer → header 17.33; above a first section without a header 35.33 |

`.grouped` has the same metrics, with full-width cells, no corners, and full-width separators above and below each section. `.plain` has a white background, full-width cells and no gaps between sections. UIKit pins a plain list's headers and footers to the visible bounds, and the iOS 26 scroll-edge effect shades content under a pinned footer.

## 3. Design

**D1. The switch: a `list` whose `appearance` is the literal `auto`.** The switch is the same one LLP 1069.011 D1 uses for buttons: CSS's own name for "the platform's control".
- The attribute must be a literal, after class rows, because it decides what the node's children are. A bound value is refused (`lower-grouped-list`). An app that switches between a grouped list and a plain list writes `when`.
- Unlike `button`, `list` gets no fixed `appearance: none` row. A plain `list` keeps the rows it has today, and the switch reads the authored attribute, not the computed row. `tags.rs` is at its line cap, and a fixed row on every `list` would change every list's `layout` output to say nothing new.
- A grouped list is never `virtualized` (refused). It builds every row, as a settings screen does. A long feed stays a collection.

**D2. `listStyle`: UIKit's three appearances.** `inset-grouped` (the default), `grouped` and `plain`.
- It is a literal, because the sheet Contract writes for the other hosts is chosen when the view compiles. It is refused anywhere but a grouped list.
- It is a Contract attribute and the kernel's prop 225 (`str`), outside LLP 1081's naming rule (its D5), like `buttonStyle`.
- Contract always writes it, so a host finds a grouped list by its prop (`ChromeIndex`'s `listStyle` key). There is no scan.
- `sidebar` and `sidebarPlain` are left out (§7).

**D3. The shape: sections, each a header, rows, a footer.** Contract checks it and refuses anything else with `lower-grouped-list`.
- **The list's children are `section`s**, possibly under `when`, `each` and `match`.
- **A section's first child may be a `header`, and its last child may be a `footer`.** Each must be a direct child, at most one of each, holding a `text`. A `header` or `footer` anywhere else in the section would be a row's, and is refused.
- **Everything between them is the section's rows**, under any control flow. Contract wraps them in one `column`, the group the sheet paints. The kernel reads that column's children as the rows. A row is any element: a `button` is tappable, and a `row` or anything else is not.

This is HTML's own structure (`section` > `header`, the rows, `footer`), so the web reads it as written.

**D4. A row's parts, by shape, read live by the kernel.** `Kernel::grouped_list(view)` walks the tree as it stands, so a `when` between parts counts. It reads a row's direct children, outside in:
1. **The trailing accessory, the last child:**
   - an image of `chevron.forward` or `chevron.right` is the disclosure indicator;
   - an image of `checkmark` is the checkmark;
   - a checkbox or switch `Control` is a toggle, which flips that control;
   - a `button` holding only an `info.circle` symbol is the detail button, which presses that button.
2. **The leading symbol, the first child:** an image whose source is a `symbol:`.
3. **What is left** (children with `display: none` are not counted, and neither is a hidden section or row):
   - one `text` is the title;
   - two `text`s are the title and its value (UIKit's value cell);
   - a `column` (a flex column: what Contract's sheet styles) of one or two `text`s is the title over its subtitle (the subtitle cell). A `row` or plain box of texts is custom.

A row of any other shape is **custom**: a raster image, a third text, a nested box. A row with no title is custom too. A custom row carries only its node, its press and its state, and the host shows its views. `destructive` (the existing prop 68) draws the title and symbol red, and `disabled` dims the row and makes it untappable. Roles name symbols as everywhere else, so `forward-chevron` is `chevron.forward`. This RFC adds the role `info` (`info.circle`), so the detail button has a web path.

**D5. iOS: UIKit's list over the authored scroll.** `GroupedListHost` (`IOS/GroupedListIOS.swift`) runs after each batch:
- **It reads the model through `exact_grouped_list`**, which returns JSON as `exact_press_face` does.
- **It installs a `UICollectionView` in the list's box** and hides the authored scroll beneath it. The layout is a compositional layout per section, so a section has a header or footer only where it has the text. The data source is a diffable data source keyed by the section and row node ids. A row whose parts changed is reconfigured, never reloaded.
- **A standard row** is a `UIListContentConfiguration`: `.cell()`, `.valueCell()` or `.subtitleCell()`. Its image is `UIImage(systemName:)`. Its accessories are `.disclosureIndicator()`, `.checkmark()`, `.detail` (which presses the button), or a `UISwitch` as a custom-view accessory. The switch shows the control's committed `checked` and reports through the control's own path (`presenter.checked`), and the committed state is authoritative, as in LLP 1069.001 D4.
- **A tap** is the collection view's own selection. UIKit highlights the row, deselects it, and presses it. Rows that are not buttons, or are disabled, do not highlight.
- **A custom row's node is carried into its cell** and given back before every batch, as LLP 1008 §9 carries a swipe row's.
  - It sits at its place in the group: its kernel x, top at 0.
  - The cell is the row's kernel height less the row's bottom border. The cell's content view clips, so the separator the cell draws replaces the sheet's.
- **Insets follow the authored scroll.** The collection view copies that scroll's `contentInsetAdjustmentBehavior`, `contentInset` and indicator insets.
- **A plain list's footers are not pinned.** UIKit pins them by default and shades the rows under them (§2); a settings footer belongs under its rows.

**D6. Reuse, and what is not reused.** The list is still a `list` node, laid out by the kernel, mounted by the presenter, with its scroll, its `testId` and its handlers. The projection follows the swipe cell's pattern: `prepare()` before a batch, `sync(changed:)` after it, hosts registered with the inspection walk. A virtualized collection's window is not reused, because a grouped list has no window (§1). The two never meet: D1 refuses `virtualized`.

**D7. The other hosts draw the sheet.** Contract adds rows to the list, the sections, the header and footer, the group and each row's parts. They are prepended to the author's rows, so a class or attribute of the author's replaces any of them. Every host lays them out as it lays out any rows. The web, macOS and Linux need no code, and on iOS the same layout is what the hidden scroll holds.
- **The list:** the grouped or plain background.
- **A section:** margins of 0, 17.33 or 35.33 (§2).
- **The group:** the cell colour; with `inset-grouped`, 16 pt margins and radius 26; with `grouped`, top and bottom separators. It always clips.
- **A row:** a flex row, 52 pt tall at least.
  - It starts at the text: 16 pt in, or 56 pt after a leading symbol. The symbol is pulled into the margin (`margin-left: -40`).
  - Its 1-pt separator is its bottom border, and it overlaps the next row by that width (`margin-bottom: -1`). The group clips away the last row's separator, so no row needs to know it is last, even under `each`.
- **Precedence:** the sheet's rows go under the element's classes and its own attributes. The rewrite marks them (`ua:`, a name no author can write), and the lowering takes them out before it expands classes (`grouped::split`).
- **The inset follows a leading symbol shown by a condition** (`margin-left` becomes a choice on that `when`). A `match` or `each` in first place is styled as no symbol, its image included.
- **A text after a `when` is styled by that `when`'s condition.** Where the arms write different numbers of texts, its `flex-grow` and colour become choices on the condition, so it is the title or the value as the kernel reads it. Deeper conditions and a `match` count the fewer texts. A text written `display="none"` is not counted.
- **A subtitle stack is the shape the kernel reads.** It is one `column` of one or two texts, a `when` showing the second line included, between a leading symbol and a trailing accessory (each possibly under a `when`). Any other `column` is the author's.
- **A native button row** (`button appearance="auto"`, LLP 1069.011) gets only the rows a native button takes (its inset and height). The kernel reads it as custom, so iOS carries the platform's control.
- **A plain list's sections have no margins.** A header or footer holds exactly one `text`.
- **The parts:** 17 pt label colour, red when `destructive` (a bound `destructive` becomes a choice); the value and subtitle in the secondary colour; the accessory's size and colour; a subtitle stack's 15-pt padding. A `when` is read through.

Colours are `light-dark()` pairs of §2's values.

**D8. The agent.**
- `tap` on a row UIKit draws is the cell's own selection (`delivery: host-activation`, `native: grouped-list`).
- `tap` on a toggle's control flips the cell's switch, and `tap` on a detail button is the accessory's action.
- Each is refused, having done nothing, when the cell is outside the list's port or something covers its middle, as a finger would miss it. The generic viewport check, which reads the hidden row's place, is skipped for nodes a grouped list draws.
- A wheel on the list, on a row it draws or on a carried view scrolls the collection view, wherever the hidden node lies.
- A row whose node or ancestor (its section included) is `inert` takes no tap: its cell takes no touch and is no accessibility element. The agent's tap is refused under the software keyboard, as `Agent.obscured` refuses one.
- A custom row is its own views: the agent's ordinary tap path finds them, a nested control included.
- `layout <list>` reports `groupedList: {view: UICollectionView, listStyle, sections, rows, offset, content, insets}`, and a row reports its cell and accessory.
- The inspection walk (LLP 1080.001 D3) accounts for the collection view (`grouped-list`), names this host as the hider of the authored rows, and skips a carried row as projected.

## 4. What is declared, not fixed

- **The sheet is iOS 27's inset-grouped look on every other host.** It is not macOS's grouped form or a GTK list. Like the native-button looks, it is a stated approximation.
- **Where the fallbacks differ from UIKit (§6's comparison sheets).** One gap is fixed; the rest are declared.
  - *Fixed: Linux draws symbol roles.* The paint walk strokes a portable role's path the way the web does (`paint/svg.rs` `symbol`): the leading icons, the chevrons, the checkmark and the info button. Before this, Linux painted every `symbol:` image as an empty box. An `sf/` name still draws nothing there (LLP 1035.004.000 D4). This applies to every app on Linux, not only grouped lists. Tested in `host/linux/tests/it/svg.rs`.
  - *Declared: the web's switch is a checkbox.* Chrome has no `switch` attribute, and LLP 1069.001 D1 specifies the checkbox fallback. Drawing a 51 × 31 track in `index.html` would be a few lines of CSS. But it would move every switch's box on the web away from the Chrome-oracle table (`kernel/tests/it/browser_controls.rs`) and the kernel's 13 × 13 default. That is a change to form controls, not a grouped-list fix, so it is queued.
  - *Declared: the web's row titles look heavier than UIKit's.* The sheet sets no weight, so a title is `system-ui` at 400. The agent's web screenshots are 1× rasters, while UIKit's are 3×. This was checked in the CSS, not against a 2× capture.
  - *Declared: the web's chevrons are faint.* A role symbol is a stroked path in a 24-unit box (LLP 1035.004.000). In the chevron's 14-pt box the stroke is about 1.4 px. A larger box would draw Apple's own glyph too large on macOS, which fits `chevron.forward` to the box.
  - *Declared: Linux's section headers look bold.* A header is weight 600, UIKit's semibold. The Linux host's pinned face, DejaVu Sans, has no semibold, so 600 draws Bold.
- **The sheet is fixed at 17 pt.** Dynamic type is UIKit's, on iOS.
- **The sheet has no pressed highlight.** A web row is a `button` and keeps the button's own focus ring.
- **Symbols on the web are role paths in a 24-unit box** (LLP 1035.004.000), so a sheet-sized chevron draws a little smaller there than UIKit's glyph. An `sf/` name draws nothing on the web or Linux, as everywhere.
- **A custom row's width is the kernel's.** In a wider UIKit layout margin (an iPad, landscape), its views keep the sheet's 16-pt inset, while UIKit's own cells follow the margin.
- **The large-title collapse (LLP 1075.003 §3.7) does not follow a grouped list.** It follows the route's authored scroll, which this list hides. The route's `setContentScrollView` would need to be the collection view (§7).
- **Swipe actions (`swipeContent`) on a grouped list's rows are not projected into its cells.**
- **A part hidden by a class's `display: none` is counted by the sheet.** The sheet sees only a literal `display="none"` written on the part. A class is applied when that part is lowered, after the row's parts are counted, so the kernel and the sheet can disagree about which text is the title. Write the attribute, or use `when`.
- **The 35.33-pt first gap goes only to a section written first.** A first section under `when`, `match` or `each` may share its body with others or follow nothing, so it gets the 17.33-pt gap.
- **A custom row takes no cell highlight.** Its carried views keep their own touch handling, so its press, press feedback and nested controls stay the author's.
- **The list's scroll position is UIKit's.** An authored `scrollTop` write still goes to the hidden scroll, and the collection view reports no `scroll` event. The kernel's content height is the sheet's, not UIKit's, so mirroring one offset onto the other would be wrong at both ends. A settings screen needs neither. The agent's wheel scrolls the collection view (D8). A consumer that needs a position gets it designed then.

## 5. Scope

This adds to `rules/DEFERRED.md` as Charlie approved, without a take: one tag meaning (`list appearance="auto"`), one prop and one symbol role. There is no new node type, tag, event or agent operation.

## 6. As built

- **Contract.** `grouped.rs` holds the checks and the sheet. `lib.rs` has the hook and the precedence split, and `tags.rs` gains `listStyle`; one comment was folded to stay at the cap. `contract/cli/tests/it/grouped_list.rs` has 15 tests:
  - the kernel's model, read live;
  - the sheet's metrics against §2 (rows 52 apart, the text at 72 after a symbol and at 32 without one);
  - the author's attributes and classes over the sheet;
  - conditional symbols, texts and subtitle lines in both states;
  - hidden parts, plain sections, native button rows;
  - each refusal.
- **Kernel.** `grouped.rs`, `Kernel::grouped_list`. Adding a prop and a role moved `SCHEMA_DIGEST`'s snapshot (`wire/codec.rs`).
- **Apple.** `exact_grouped_list` and `GroupedListIOS.swift`. `GroupedListIOSTests` has 13 tests:
  - the collection view in the box over the hidden scroll;
  - cells of the model's parts;
  - the tap: highlighted, pressed once, refused when scrolled away or inert;
  - a switch following its control's state and target, and one rebuilt only after its own action;
  - custom rows carried, given back in order, turning custom and back, and keeping their section's inertness;
  - a header's text changing;
  - the wheel;
  - a list that goes.
- **Fixture.** `scripts/fixtures/grouped-list.contract` is a settings screen with a profile card (custom), symbol rows, a value, a subtitle, a detail button, a switch, a checkmark choice, a destructive row, and a segmented control for the three styles.
- **Seen.** The fixture was driven beside §2's scratch UIKit app, in light and dark, on the iOS 27 simulator, the web (Chrome) and Linux.
  - On iOS the agent tapped rows, the detail button, the switch and the choice, and wheeled the list to tap a row below the fold.
  - Plain footers stay under their rows.
  - macOS draws the same sheet Linux and the web do; it was not driven.
- **Review.** Three rounds each by Astra and Grok (`llp/reviews/code-2026-10-03-grouped-list.*.md`). Round 3 ended with Astra at DO NOT LAND and Grok at LAND WITH FIXES. Every round-3 finding is fixed or declared in §4.

### 6.1 The symbol's tint (2026-10-04)

The sheet takes the author's attributes over its own (D7), but UIKit's cells
did not. Signal Clone's settings symbols came out in UIKit's accent colour
rather than the label colour it wrote.
- **A standard row's symbol** takes the tint the sheet draws: the author's
  `tint-color` over the sheet's own `light-dark(#0088ff, #0091ff)`. It is a
  dynamic colour, so each side of a `light-dark()` pair follows the
  appearance with no batch.
- **The symbol** is the one the model named: the row's first shown child,
  when it is that symbol's image, as `kernel/src/grouped.rs` reads it.
  `display: none` children are skipped.
- A row whose symbol tint changes is configured again. A destructive or
  disabled row's colour still wins.
- A custom row's background is not changed: on the sheet a transparent row
  shows its group's cell colour, and so does the cell (r1 cleared the cell,
  and both reviews found that not to be parity).

`GroupedListIOSTests.testASymbolTakesItsAuthoredTintForEachAppearance` covers
a hidden image before the symbol, a chevron after it, and both appearances.

## 6.2 A section without its card (2026-10-04, Signal Clone)

Signal's profile header and conversation-settings header sit on the list's
background, not on a card: a section whose cell background is clear. The
author writes `section background-color="transparent"`.
- **Contract.** That literal is the only `background-color` a section takes
  (`lower-grouped-list` refuses another value, and a `class` beside it): a
  coloured card would be the system's card on iOS. The group the sheet
  paints as the card is then transparent, its rows draw no separators, and a
  `grouped` list draws no borders around it.
- **Kernel.** `GroupedSection.card` is false when the group's background is
  transparent; `exact_grouped_list` carries it as `"card"`.
- **iOS.** That section's cells take a clear background, and a pressable
  standard row still shows UIKit's highlight while pressed (a configuration
  update handler). Its layout shows no separators (iOS only: tvOS has no
  `showsSeparators`), and keeps the appearance's list background: the inset
  card is the cells' background, not the section's (a clear section
  background showed the route's white, simulator, 2026-10-04). A custom row
  keeps its full height, as no UIKit separator stands in for its border. A
  row whose card changed, its section's or by moving, is configured again,
  and the list is laid out again.

Proofs: `contract/cli/tests/it/grouped_list.rs`
`a_transparent_section_has_no_card` and
`a_cardless_section_draws_no_separators_and_takes_only_transparent`;
`GroupedListIOSTests.testACardlessSectionsCellsAreClear`.

## 7. Open

- **`sidebar` and `sidebarPlain`.** These are UIKit's other two appearances. They wait for an iPad or Mac consumer.
- **The large title.** The route's content scroll view (LLP 1075.003 §3.7) should be the collection view when a grouped list is the scroller after the header.
- **Swipe actions on rows.** `UICollectionLayoutListConfiguration`'s own `trailingSwipeActionsConfigurationProvider` would replace the one-row table for a grouped list's rows.
- **The Signal Clone.** Its `Section`, `Row` and `ActionRow` become a grouped list. That is a change in the clone's repository, not this one.
