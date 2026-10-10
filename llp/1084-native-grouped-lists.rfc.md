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

**D5. iOS: UIKit's list is the scroll owner.** `GroupedListHost` (`ExactGroupedLists/GroupedListIOS.swift`) runs after each batch:
- **It reads the model through `exact_grouped_list`**, which returns JSON as `exact_press_face` does.
- **It installs the list's only `UIScrollView`, a `UICollectionView`, in the list's box.** The original section roots remain in the authored hierarchy and are hidden by the host; no duplicate scroll or wrapper is created. The layout is a compositional layout per section, so a section has a header or footer only where it has the text. The data source is a diffable data source keyed by the section and row node ids. A row whose parts changed is reconfigured, never reloaded. An unchanged snapshot is not reapplied; custom rows refresh their carried views and height during mounting without reconfiguration.
- **A standard row** is a `UIListContentConfiguration`: `.cell()`, `.valueCell()` or `.subtitleCell()`. Its image is `UIImage(systemName:)`. Its accessories are `.disclosureIndicator()`, `.checkmark()`, `.detail` (which presses the button), or a `UISwitch` as a custom-view accessory. The switch shows the control's committed `checked` and reports through the control's own path (`presenter.checked`), and the committed state is authoritative, as in LLP 1069.001 D4.
- **A tap** is the collection view's own selection. UIKit highlights the row, deselects it, and presses it. Rows that are not buttons, or are disabled, do not highlight.
- **A custom row's node is carried into its cell** and given back before every batch, as LLP 1008 §9 carries a swipe row's.
  - It sits at its place in the group: its kernel x, top at 0.
  - The cell is the row's kernel height less the row's bottom border. The cell's content view clips, so the separator the cell draws replaces the sheet's.
  - A prepared cell carries the row again when UIKit displays it; a batch may have restored the row after cell preparation. A changed height requests the existing coalesced projection refresh, without reconfiguring or invalidating inside the display callback.
- **Scroll configuration uses the shared owner.** The node's `scrollView` is the collection. Refresh, keyboard dismissal, indicator style, authored offsets, events and navigation use it directly; the collection delegate forwards the shared scroll callbacks while retaining native row selection. UIKit owns its content extent. Navigation registers the physical backend before its first layout, and shared offset writes follow navigation and keyboard inset adoption. An immediate logical write that changes the collection's title inset settles that transition once before its final target; it retains no later offset replay. A plain scroller's extent remains the kernel's.
- **Content room stays content.** Native insets implement the list's top and bottom padding and the space under a footer, as this module's own delta. Shared scroll metrics, focus reveal and alignment exclude that room from UI obstructions; the kernel's scroll facts do not add authored padding twice. Backend replacement retains the logical offset and transfers an existing keyboard-toolbar inset after mounting.
- **Geometry resolves by row identity.** A standard or offscreen row's native layout attributes supply its projected box; a carried custom descendant supplies its own box. Reveal and reading anchors use those boxes rather than the hidden section positions. Navigation binds after mounting and compares the physical scroller identity. A fractional difference within 0.5 points at the native start is not a reading anchor; an idle follow-end list at that start uses the new native minimum when its inset changes.
- **A plain list's footers are not pinned.** UIKit pins them by default and shades the rows under them (§2); a settings footer belongs under its rows.

**D6. Reuse, and what is not reused.** The list is still a `list` node, laid out by the kernel, mounted by the presenter, with its native scroll, its `testId` and its handlers. The projection follows the swipe cell's pattern: `prepare()` before a batch, `sync(changed:)` after it, hosts registered with the inspection walk. A virtualized collection's window is not reused, because a grouped list has no window (§1). The two never meet: D1 refuses `virtualized`.

**D7. The other hosts draw the sheet.** Contract adds rows to the list, the sections, the header and footer, the group and each row's parts. They are prepended to the author's rows, so a class or attribute of the author's replaces any of them. Every host lays them out as it lays out any rows. The web, macOS and Linux need no code, and on iOS that layout remains on the original sections and determines custom-row sizes.
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
- **Swipe actions (`swipeContent`) on a grouped list's rows are not projected into its cells.**
- **A part hidden by a class's `display: none` is counted by the sheet.** The sheet sees only a literal `display="none"` written on the part. A class is applied when that part is lowered, after the row's parts are counted, so the kernel and the sheet can disagree about which text is the title. Write the attribute, or use `when`.
- **The 35.33-pt first gap goes only to a section written first.** A first section under `when`, `match` or `each` may share its body with others or follow nothing, so it gets the 17.33-pt gap.
- **A custom row takes no cell highlight.** Its carried views keep their own touch handling, so its press, press feedback and nested controls stay the author's.
- **Native and authored geometry are distinct.** The kernel still lays out the sheet and its custom rows; UIKit lays out native cells and sections. Native scroll positions, ranges and projected row boxes come from the collection, without mirroring an offset onto a second scroller.

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
- **Apple.** `exact_grouped_list` and `GroupedListIOS.swift`. `GroupedListIOSTests` includes:
  - the collection view as the only scroll owner, with hidden authored section roots;
  - initial and same-batch scroll writes, native extents and events, refresh and keyboard style;
  - projected offscreen geometry, reading anchors and nested scroll reveal;
  - prepend/resize reading positions, native end following, padded metrics and focus reveal;
  - backend offset transfers and authored-write precedence;
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

## 6.3 A real touch on what the list draws (2026-10-04, Signal Clone)

Under `--touch platform` (LLP 1080.000), `tap` on a row, its toggle's control
or its detail button was aimed at the hidden authored node and refused
(`_UISystemBackgroundView covers its middle`). The aim (D4) now takes the view
UIKit draws for it, as D8's host activation does: the row's cell, the cell's
switch, or UIKit's detail button (a control in the cell outside its content).
It is refused when the cell is outside the list's port, when the point is,
when anything but that view (an ancestor beside a clipped cell included) is
hit there, and when the switch or detail button is not shown: never the row
in its place. The node the dispatch log must see the touch land on is the
list's for every row, so the log and the aim also carry `projected: {row,
part}` (`cell`, `switch` or `detail`) and the driver refuses a touch that
landed on another row or part. Unlike host activation it does not refuse a disabled row or control:
the finger lands, and UIKit declines it, as on a device. A custom row needs
nothing: its views are carried into the cell.

Proofs: `GroupedListIOSTests.testARealTouchAimsAtTheCellOrAccessoryUIKitDraws`;
`scripts/smoke-touch.mjs` taps `contract/corpus/grouped-touch.contract`'s
switch twice, its detail button and a row with real touches, each read back
from the app's state.

## 6.4 A section's authored spacing (2026-10-04, Signal Clone)

Signal's tables put 20 points between sections (`OWSTableViewController2`'s
`defaultSpacingBetweenSections`), where UIKit's list leaves 35. The author
writes the section's margins, as on the web: `section margin-top=20
margin-bottom=0`.
- **The sheet's numbers are UIKit's.** A margin of 0 beside a header or a
  footer or in a plain list, 17.33, or 35.33 above a first section, is the
  sheet's (§2), and iOS keeps UIKit's own gap there (its last footerless
  section has 20 below it, which the sheet's 17.33 stands for). Any other
  number is the author's: a margin written as exactly one of those numbers,
  where the sheet writes it, reads as UIKit's (a first section's authored
  17.33 keeps UIKit's 35). The kernel cannot tell an authored value from the
  sheet's once lowered; carrying that provenance is open (§7).
- **Kernel.** At a boundary where either margin is the author's, the space
  is the web's: the two margins collapse as block margins do, the largest
  positive plus the most negative (30 and 40 are 40, not 70; 30 and -10
  are 20).
  `GroupedSection.space_above` carries it, `GroupedList.space_below` the
  last section's authored `margin-bottom`; `exact_grouped_list` writes
  `"spaceAbove"` and `"spaceBelow"`. Only points count; a percentage is 0.
- **iOS.** The whole space goes above the later section, and the earlier
  one's bottom inset is 0 unless it has a footer (that inset is UIKit's gap
  between its rows and the footer). Above a header the space is the list
  configuration's `headerTopPadding`: a section's top inset under a header
  is the header-to-rows gap (measured, iOS 27). The space under the last
  section is its bottom inset, or under a footer the collection's bottom
  content inset (added as this module's own delta, preserving refresh and keyboard insets).

Proofs: `contract/cli/tests/it/grouped_list.rs`
`an_authored_margin_is_the_webs_space_and_the_sheets_is_uikits` and
`authored_margins_collapse_as_the_web_lays_them_out` (the kernel's own
layout agrees; no browser was run) and
`a_negative_margin_collapses_as_css_has_it`;
`GroupedListIOSTests.testAnAuthoredSpaceSitsAboveTheSectionAndItsHeader` and
`testAnAuthoredSpaceUnderTheLastSectionIsUnderItsFooter`; the Signal clone's settings sheet on the iOS 27 simulator (20-point
gaps, a titled section's header 20 under the card above).

## 6.5 A large title over the one scroller (2026-10-09, Primitives)

D5 makes the collection view the list's only scroller and binds it to the route as `collapse` runs. The title's lifecycle on top of that (ported from the head of draft #350, which kept a hidden authored scroll under the collection view; here there is one physical scroller, the node's `scrollView`, and nothing else is given):

- **A later tab** (26.5). A bar first laid out in the window over a scroller it already follows rests with its large title collapsed: a tab selected after launch over a grouped list or a `scroll` showed the inline title over content at its start. A route never laid out in a window takes its scroller the turn after it is first laid out in, or appears in, the window (`RouteController.track`, `viewDidLayoutSubviews`, `viewIsAppearing`); one laid out takes it at once. `viewDidAppear` is too late: a tab's selection animation holds it back past an authored `scrollTop`.
- **The bar takes a scroller no finger has moved** (26.5): it sets its height from the scroller's place but not its titles. So a fix-up is owed (`RouteController.settle`) once the route shows on top of its stack with no push or pop in flight. Over a scroller at its start the bar is laid out and only ever grown to its large title (on 27.2 a large bar sized to fit collapsed over a list at its start after a push and a pop, and with no scroller it is hidden and shown again, which lays it out large). Over a scrolled one a bar still at its large height stays so, as UIKit's own; below it, `largeTitleDisplayMode` goes to `.never` and back, which sets its titles again.
- **A node put between the header and the list** (26.5 and 27.2). The bar stops following the list, and UIKit leaves the large title inside it, clipped: no title showed. The bar is laid out at once when its scroller changes (`RouteController.give`) and takes the title back; with no scroller it rests large. Taken out, the bar follows the list again from its start.
- **A list that arrives while its tab is hidden** (26.5): its route has been laid out, so it gives the new collection view at once, and the list keeps its start when the shown tab's bar adds room above it.
- **The list keeps its start.** A collection view, unlike a `scroll`, does not keep its offset at its start when the room above it changes. `GroupedCollectionView.adjustedContentInsetDidChange` keeps a list at rest at its start under any new top inset, its own padding (`mount`, which now relies on it) or its window's and bar's; a drag, a fling or a bounce is left alone, and room below is not counted.
- **A scroller replaced** (26.5 and 27.2). A new scroller put under the bar (a `when` that swaps the list for a `scroll`) started at UIKit's offset 0, so the bar's room put its CSS start 168 points up: it rested scrolled, its title collapsed. `collapse` now keeps a scroller's CSS position as it binds it (`contentInsetAdjustmentBehavior` turns `.always`), so a new one starts at its start, as a browser's does at `scrollTop` 0. A pending authored write still lands after the bind.
- **A pop** (iPad, 26.5 and 27.2). UIKit takes a popped route off its stack, and nils its `navigationController`, as the pop begins; `reportCovers` uncovered its header, which moved its list under a bar that did not move, and after a cancelled edge swipe the bar showed no title, then drew the large title on the next pop. The route a pop takes away is covered to the pop's end. A pop back to a root at rest shows its large title at its start; to a scrolled root, its reader's place and a title.
- **A push on iPad** (26.5 and 27.2). UIKit pushes a collection view inset by the bar it is pushed from (138 points), reads it as scrolled under the pushed bar's taller large state (192) and rests inline (140); Exact pushed a list laid out under its own header and showed the large title. An animated push of a route whose title collapses with a grouped list waits until its header is covered by the bar and the list is laid out under it (`NavigationHost.holdsPush`, at most two turns, released after the batch or with the projection sync), and the list is given to UIKit as the route appears, in the safe area of the bar it is pushed from (`givesAsItAppears`, `propagateSafeArea`): inline "Pushed" at the same place as UIKit's. A `scroll` is not held: UIKit's own scroll view pushes large on iPad.
- **A push and a pop are UIKit's own** (27.2, iPhone and iPad). `setViewControllers` with one route more pushes in 12 frames where UIKit's push takes 15, and takes the large title of the route underneath off as the push starts; UIKit's push moves a copy of that title out with its route (the label itself goes clear). On iPad a pop made the same way also takes the leaving route's inline title off. One route put on or taken off the top is now `pushViewController` or `popViewController`; any other change stays `setViewControllers`. `testARootsLargeTitleSlidesOutWithItAsAPushStarts` fails without this (no copy of the root's title moves out with it) and passes with it; `testAnAppsOwnPopBackToARootAtRestShowsItsLargeTitleAtItsStart` pops through the app's own back action and finds one route left, its title large at its start.
- **A back swipe follows the finger as UIKit's does** (iPad, 26.5 and 27.2). An edge swipe held at 0.4 of the width put Exact's page 16 points further right than a UIKit app's, so the end of the root's large title showed at the screen's edge where UIKit's does not. A UIKit app whose pop recognizers' delegate is replaced does the same (149 to 156 at 410 pixels across); one that keeps UIKit's answers on how each pop stands with the other recognizers does not, and those answers come in UIKit's own messages as well as the public ones. Exact still decides whether a swipe may begin and what it is given, for a route whose bar is hidden too; each pop's delegate (`PopGestureDelegate`) now asks UIKit's own for everything else. Exact's page held where UIKit's does: 149 on the 26.5 iPad, 148 on the 27.2 iPad, 103 on the 26.5 iPhone; a route with no header still pops by an edge swipe and by one from mid-screen. `testEveryTabsPopGesturesAskExactAndAPushedScreenMayPop` checks that UIKit's delegate is kept and answers the failure relations while Exact answers whether to begin.
- **A sheet is UIKit's default sheet** (iPad, 26.5; decided 2026-10-09). Exact presented every non-fullscreen modal route as a `.pageSheet`, which on an iPad spans the width; a UIKit app that names no style gets `.automatic`, a card in the middle (580 by 650 points on an 11-inch iPad, its bar and large title at the same place in both now). `ModalController` asks for `.automatic`: on iPhone the same page sheet as before, with its detents; on an iPad the card, where UIKit leaves detents to compact widths. On 26.5 and 27.2 the card's bar, Back and large title are at the UIKit app's places; on the 26.5 iPhone the page sheet, and a route pushed in it, are too. On 27.2 a 580-point inset-grouped list in the card kept 8 points on its trailing side where UIKit's keeps 16. The list section takes its side insets from the collection view's content margins (`_UIContentInsetsEnvironment`'s `contentMargins`), which UIKit infers from the superview when a view's own geometry changes: the route's node had been sized for the card while its controller's view was still 820 points wide, so it took 16 and 0, and kept them when that view shrank to 580. `RouteController` now has UIKit infer the node's margins again when its view's size changes (`inferMarginsAgain`, through `insetsLayoutMarginsFromSafeArea`), and on the 27.2 iPad (A16) the rows are at the UIKit app's place, 16 and 16; on the 26.5 iPad both are 20 and 20 still. A second copy of the large title placed 52 points low in the 26.5 card's accessibility tree was not seen again: both apps list one title, at one frame. `testARouteNodeTakesItsControllersContentMarginsAgainAsItsViewResizes` drives the inference (27.2; UIKit 26.5 has no content margins). The system also listed a "Vertical scroll bar, 1 page" for Exact's viewport over every screen, where the UIKit app lists none; the viewport, which a fitting document neither scrolls nor bounces, now shows a bar only on an axis that scrolls (`testAViewportShowsAScrollBarOnlyOnAnAxisThatScrolls`). A list's own scroll bar was listed 52 points lower at rest than the UIKit app's (iPhone and iPad, 26.5), until the first scroll: UIKit sizes the bar from insets it caches when the scroller's safe area changes, the large title it holds left out, and a list given its bar after its last change kept insets that counted the title. `RouteController.settle` now has them computed again (`measureScrollBarAgain`, through a changed indicator inset set back at once); at rest both apps list the bar at one frame, and during a drag both indicators start at 89 points (`testAListAtRestUnderItsBarHasItsScrollBarMeasuredForItsTitle`). Closing the card showed the screen under it wrong for the whole dismissal, about half a second (27.2 iPad): the tabs 120 points right of centre, the bar's Edit gone, the rows 12 points wider, where the UIKit app's screen holds still. While a sheet is up, the route node its background (the tabs) belongs in is laid out at the sheet's size, 580 by 660 points; the background went back into it at its old frame and then grew with the node, by 240 by 520 points to 1,060 by 1,700, until the dismissal's sync set it again. It now goes back at the node's size at that moment, its margins kept (`ModalHost.returnFrame`), and grows with the node to the screen's size: in a recording of the close, the bar keeps its Edit and its tabs in every frame, as the UIKit app's does (`testASheetsBackgroundReturnsToItsNodeAtTheNodesSize` checks the rule with views; the close itself waits for a presentation transition the test window never finishes). With the card up, UIKit's own VoiceOver traversal (`_accessibilityLeafDescendantsWithOptions:` with `defaultVoiceOverOptions`) reaches the card's Back, title, section and rows in both apps, at the same frames, and nothing behind the card (27.2, iPhone and iPad): UIKit marks its presentation container modal, and Exact also hides the background. Argent's accessibility reads and XCUITest list the background in the UIKit app; they do not read the modal flag.
- **A focus UIKit gives back** (iPhone, 26.5). UIKit resigns a field that has the focus as its route leaves the window (its tab is left, a route is pushed over it), gives the focus back as the field enters the window again (`_promoteSelfOrDescendantToFirstResponderIfNecessary`, in the call that adds the view), and scrolls it into view itself (`scrollTextFieldToVisibleIfNecessary`): in a UIKit app (its title inline, its content as wide as its scroll view) the field rested at the bar's edge after its tab came back and after a pop. Exact scrolled twice more. UIKit announces the keyboard's hide inside that resign, and the reveal that followed scrolled the route that was leaving; as the keyboard came back, the reveal rested the field 8 points lower than UIKit's. A keyboard that hides now reveals nothing, and the reveal leaves a focus UIKit gives back to UIKit (`FocusReturn`, held across a push, which takes the covered route out of the window, into its transition and out again). The repro and the UIKit app now rest at the same frames after both: the field at 116 points, its first row at 158. A UIKit app whose scroll view's content has no width (a stack's sides pinned to the frame guide) scrolls nothing for such a field, as `scrollRectToVisible` gives up outside the content; that made the first comparison read as "UIKit does not scroll". `testExactAddsNoScrollToAFocusUIKitGivesBackAsItsTabReturns` and `testAFocusReturnOutlastsAPushesTransition` check Exact's part. `testEveryTabKeepsItsStackItsScrollAndItsDraftAndReselectPopsToRoot` now ends its edit before the tab leaves: in a full run on the 26.5 simulator, with the software keyboard shown, its draft was given back and scrolled into view.
- **A tab selected again over a scrolled plain scroll view, or popped back to** (iPhone, 26.5 and 27.2). The UIKit app's bar comes back inline with its title, the reader where it was (62 to 116 points, the same offset), over a plain scroll view as over a collection view; Exact does the same (`testATabSelectedAgainOverItsScrolledListKeepsItsReaderAndShowsATitle`). An earlier comparison read a bar as tall as a large title with no title in it, and the content 52 points lower, as UIKit's: that came from the reference app's scroll view, whose content had no width (its stack's sides pinned to the frame guide); with its content as wide as the scroll view the bar stays inline. With two items after the title, UIKit's inline title sits at the leading edge, in both apps.

**Rotation stays as recorded.** A grouped list's collection view is UIKit's own and is left to do as UIKit's does after a rotation: inline on 26.5, large on 27.2. Checked again with the final build against a UIKit app turned the same way (landscape, then portrait), its scroll view's content as wide as the scroll view: on the 26.5 iPhone over a grouped list at its start both rest inline, the title in the bar at the same place (0.414, 0.084 of the screen) and the section at 0.133. Over a `scroll` at its start they differ on 26.5: the UIKit app rests inline, its content 52 points up (its first button at 0.151 of the screen), and Exact rests large (the title at 0.137, the first button at 0.211). The fix-up owed is at the end of a size transition for a route that follows a `scroll` (`viewWillTransition`). An earlier check read both as large, and before it UIKit's as the one that expands: that UIKit app's content had no width (a tab selected again, above). On the 27.2 iPhone both rest large over a `scroll`. On the 27.2 iPad (A16), with the card open over a grouped list, a turn to landscape and back keeps the card 580 by 660 points in both apps, its Back, title, rows and scroll bars at the UIKit app's frames. Once the app has been in the background with the card open, a turn can leave the card 640 points tall instead, in landscape and back in portrait, at the same frames in both apps, until the app goes to the background again. It does not follow the steps alone: a new process each time, the card opened, Home, back to the app and a turn and back gave 660 and then 640 in the UIKit app, and 640 in Exact (660 when the Mac was loaded); a card opened after the return stays 660. UIKit's cached frame (`_frameOfPresentedViewInContainerView`) is the only difference in its form sheet controller between 660 and 640, and 580 by 640 is the default card size UIKit gives this screen (`_formSheetSizeForWindowWithSize:screenSize:`). That is UIKit's own, so it is not a fault to fix in Exact. At 660 points the sheet's layout info (`UIKit.SheetLayoutInfo`) holds a preferred size of 580 by 640 and a 20-point top margin (`additionalMargins`); a 640-point state was not caught again in three tries after a reboot, so which input it drops is not shown. A card opened by a tap starts about 28 ms after the UIKit app's (the time to half its dim: 229 ms against 201, on a loaded Mac): Exact builds the route on the main thread before UIKit's presentation starts (`NavigationHost.sync` and `GroupedListHost.sync`, 55 of 64 sampled ms); queued (`QUEUE.md`).

Proofs: `scripts/fixtures/native-navigation-scroll.contract` adds three tabs (the cold list first, a later tab whose list can be swapped for a `scroll` and can get a node above it, a tab whose list arrives while hidden) and a pushed route, and `NavigationCollapseIOSTests` drives them through the compiled plan: `testALaterTabRestsWithItsLargeTitleOverItsListsStart`, `testAListThatArrivesWhileItsTabIsHiddenIsFollowedFromItsStart`, `testANodePutBetweenTheHeaderAndTheListAndTakenOutKeepsTheTitle`, `testReplacingTheScrollerUnderTheTitleFollowsTheNewOneFromItsStart`, `testAPopBackToARootAtRestShowsItsLargeTitleAtItsStart` and `testAPopBackToAScrolledRootKeepsItsReaderAndShowsATitle`. Without this section's changes four fail on the iOS 26.5 simulator (the later tab, the arrival, the node between, the replacement) and two on 27.2 (the node between, the replacement); with them all six pass on both. A repro app on the 26.5 iPhone simulator showed a title in each case where the stack alone showed none or the inline one (later tabs over a `scroll` and over a grouped list, a node put between and taken out, a push and a pop), and on the 26.5 iPad pushed inline at UIKit's place and kept the inline title through a cancelled edge swipe and the back control after it. A pop's edge swipe and a held push need real touches and an iPad, which these tests do not have. On the 27.2 iPad they were recorded against a UIKit app driven with the same touches (a push, two cancelled edge swipes, a completed one, a push from the collapsed title, a swipe back): with the push above, each phase shows the titles UIKit's does and takes UIKit's frames, within one.

**An authored write to the end under a large title** (26.5 and 27.2). A plain scroll's write is aimed short of the end by the title's collapse range (`scrollOrigin` less `scrollCollapsed`), which is known only once the title has rested collapsed. A later tab that rests large, as it now does on 26.5 and always did on 27.2, has never collapsed: the write landed on the end, the title collapsed by 52 points, UIKit moved the offset with the inset, and the scroll rested 52 points past its end (`testFullHeightPlainScrollOwnsTheNativeBottomInsetAndReachesItsEnd` failed on 27.2 before this section's changes; on 26.5 it passed only while the tab rested inline). `writeScrollPosition` now settles that one inset transition for a plain scroll as for a collection: the same intent is aimed once more with the new inset. `testPlainShortContentRefitsWhenTheToolbarChangesAndKeepsManualInsetRoom` also failed on 27.2 with and without these changes: there a tab's toolbar adds no bottom room (83 points shown and hidden, 86 and 83 on 26.5), so the inset it waited to see change never does. It now measures that room first and, where there is none, checks only the refit. `NavigationCollapseIOSTests` passes 33 of 33 on the 26.5 and 27.2 iPhone simulators.

## 7. Open

- **A real touch's identity when a row's control is replaced.** §6.3's `projected: {row, part}` tells rows and parts apart, not the authored control behind one switch: a `when` that replaces a row's control between the aim and the touch passes as the old one (the switch the finger meets is the same one). Carrying the target id needs the host's model in the dispatch log's landing (astra, round 3 of `code-2026-10-04-grouped-platform-tap`, deferred).
- **A section margin's provenance.** §6.4 reads the sheet's own numbers as
  UIKit's; Contract could carry which margins the author wrote instead.
- **A grouped list made a flex container.** §6.4 collapses section margins
  as a block list's are; a list the author writes `display="flex"` adds them
  on the web. Contract could refuse it, or the kernel follow the display.
- **A browser's boundaries.** §6.4's proofs read the kernel's layout and
  UIKit's; a fixture compared in a browser beside UIKit would close it.

- **`sidebar` and `sidebarPlain`.** These are UIKit's other two appearances. They wait for an iPad or Mac consumer.
- **Swipe actions on rows.** `UICollectionLayoutListConfiguration`'s own `trailingSwipeActionsConfigurationProvider` would replace the one-row table for a grouped list's rows.
- **The Signal Clone.** Its `Section`, `Row` and `ActionRow` become a grouped list. That is a change in the clone's repository, not this one.
