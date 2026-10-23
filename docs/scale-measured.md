# The base scale, measured (LLP 1121 stage 11 B)

Each cell of `kernel/tables/schema.json`'s `scale` that is not a reference to a metrics row names an entry here: what was
drawn or read, where, how, and the number. Each entry's `Binds:` line names every step it gives and its value (`space.xs = 4`): the kernel's build refuses a
`measured` or `guideline` cell whose id has no entry here or whose entry does not bind that step to that value, and
`kernel/src/style/scale.rs`'s tests hold the same, and every cell to its source.

**Which source a cell takes** (theme2's rulings, 2026-10-19 09:56 and 10:57):

- A metrics row only where it names the same role: a row's inset is space.md (`grouped-row-inset`), a row gap space.sm
  (`grouped-row-gap`), a control's corner radius.md (the neutral look's `control-radius`), a card's or surface's corner
  radius.lg (`grouped-card-radius`, the neutral look's `surface-radius`), a mark's corner radius.sm (the neutral look's
  `mark-radius`).
- Otherwise measured on Apple (below), Material 3's published scales for the Material column, and the neutral look's own
  steps for the neutral column (LLP 1115.000 D17: the neutral look is Exact's own, not Material).
- There is no web column: every web build draws the neutral look, so `Column::Web` (a process that says no column, and
  Material's fallback) reads the neutral cell (theme2, 2026-10-19; LLP 1115.000 D17 as corrected).
- One role stretch, ruled by theme2 with its reason (below): the Mac's space.xl. The table writes it as a stretch cell,
  `{"stretch": "=grouped-label-inset", "why": "…"}` (LLP 1121 stage 13 D5), and the build requires its entry here.
- The terminal's spacings are cells (`n ch`); it draws no corners (0). radius.full is `style::scale::RADIUS_FULL`, one
  oversized length every host clamps into a pill (theme2's C ruling).
- px (points), never scaled by Dynamic Type (ruling 6; QUEUE has the opt-in).

**How (2026-10-19):** `bun scripts/measure-scale.mjs ios|mac` builds and runs `scripts/measure-scale/ios.swift` or
`mac.swift` and prints each value.

- iOS: iOS 27.0 simulator (iPhone, 402×874 points; build 24A434), a UIKit app built with `swiftc` for the simulator.
  Values UIKit states are read from its API (layout margins, `equalToSystemSpacingAfter/Below`,
  `UIListContentConfiguration`, `UIButton.Configuration`). A shape UIKit only draws is rendered at 3× with
  `drawHierarchy`, and its corner read as the equivalent circular radius: the inked box (any ink), then the distance `d`
  along the 45° diagonal from its corner to where the ink reaches half the level of the box's top edge, `r = d / (√2 − 1)`.
  Check: the inset-grouped list's card measures **26.07** against the table's `grouped-card-radius` 26.
- macOS: macOS 27.0 (26A428), an AppKit tool built with `swiftc`. Spacing by API, read on alignment rects. Corners from
  the window server's composite of the tool's own window (`CGWindowListCreateImage`, looked up at run time as
  `AgentMac.swift` does; AppKit 27 draws a bezel as a layer that `cacheDisplay`, a cell's `draw` and `layer.render` all
  leave out), on a flat white ground, by the same diagonal; the capture is 1× on this display, so a Mac corner is good to
  ±0.7 point and is recorded as the nearest whole point.

## Measured

### `ios-button-small-inset-top`
Binds: `space.xs = 5`.
space.xs on iOS: **5**. `UIButton.Configuration.filled()` with `buttonSize = .small`: its
`contentInsets.top` (the leading is 10). The tightest spacing UIKit states between a control's edge and its content; no
UIKit API gives a 4 (system spacing is 8, a list's subtitle gap 0, a grouped header's leading margin 8). theme2 10:57 (3).

### `ios-button-large-inset-leading`
Binds: `space.lg = 20`.
space.lg on iOS: **20**. `UIButton.Configuration.filled()` with `buttonSize = .large`: `contentInsets.leading`.
The view controller's minimum layout margin is 16 on an iPhone (the same as space.md) and 20 on a regular width, which one
iPhone simulator cannot show (a trait override left it at 16); 20 is UIKit's stated step above 16. theme2 10:57 (3).

### `ios-inset-grouped-section-gap`
Binds: `space.xl = 35`.
space.xl on iOS: **35**. A `UICollectionView` list, `.insetGrouped`, no headers, two sections of one row:
from the first card's bottom to the second's top. theme2 10:57 (accepted).

### `ios-cornerstyle-small-44`
Binds: `radius.sm = 5.6`.
radius.sm on iOS: **5.6**. A filled button with `cornerStyle = .small` at 140×44 points measures 5.58.
iOS 27 has no checkbox or radio mark, so this is UIKit's smallest corner style, recorded as a FIXED length at that 44-point
height: UIKit's `.small` scales with the button's height (10.09 at 80 points tall), the base does not. theme2 10:57 (3).

### `ios-button-medium-corner`
Binds: `radius.md = 17`.
radius.md on iOS: **17**. A filled button, `buttonSize = .medium`, as UIKit sizes it (34.3 points tall):
17.04; `.bordered`, `.gray` and `.tinted` the same, and its `.dynamic` corner stays 17.04 at any width. iOS 27's controls
are capsules: this is that capsule's corner at height 34, recorded as a length, not `full`. theme2 10:57 (4).

### `mac-grid-spacing`
Binds: `space.xs = 6`.
space.xs on the Mac: **6**. `NSGridView()`'s default `rowSpacing` and `columnSpacing`: AppKit's tightest stated spacing
between controls (`NSBox`'s content margin is 5; system spacing between views 8, which is space.sm).

### `mac-window-margin`
Binds: `space.lg = 20`.
space.lg on the Mac: **20**. A view placed with `equalToSystemSpacingAfter` / `equalToSystemSpacingBelow` the window's
content view: 20 from the leading and the top edge, AppKit's standard window margin. theme2 10:57 (accepted).

### `mac-checkbox-corner`
Binds: `radius.sm = 5`.
radius.sm on the Mac: **5**. `NSButton(checkboxWithTitle:)`, regular size: its 16-point box's corner measures 5.25 (1×,
±0.7), recorded as 5. A mark's corner, the role the neutral look's `mark-radius` has.

### `mac-push-regular-corner`
Binds: `radius.md = 6`.
radius.md on the Mac: **6**. `NSButton`, `.push`, regular size (24 points tall): 6.22 (1×), recorded as 6; a pop-up
button and a segmented control measure the same. Large and extra-large push buttons are capsules (13.75 on 28, 17.68 on 36).

## Guidelines

### `neutral-steps`
Binds: `space.xs = 4`, `space.lg = 24`, `space.xl = 32`.
space.xs, space.lg and space.xl in the neutral look: **4**, **24** and **32**. The neutral look is Exact's own (LLP
1115.000 D4 part B, D17), so its steps are its own definition, chosen to match the look's existing rows (`menu-padding` 4,
`alert-padding` 24, `alert-margin` 32) without borrowing their roles; they coincide with Material 3's 4-dp spacing steps,
but the neutral look is not Material. theme2 10:57 (2).

### `m3-spacing`
Binds: `space.xs = 4`, `space.lg = 24`, `space.xl = 32`.
space.xs, space.lg and space.xl in Android's Material look: **4**, **24** and **32**. Material 3, Layout, "Spacing"
(m3.material.io/foundations/layout/understanding-layout/spacing): spacing in multiples of 4 dp; 4 the smallest step,
24 dp the margin of a medium window, 32 dp the next larger step on that grid. theme2 10:57 (5) accepted 32.

### `m3-shape`
Binds: `radius.sm = 4`, `radius.md = 8`, `radius.lg = 12`.
radius.sm, radius.md and radius.lg in Android's Material look: **4**, **8** and **12**. Material 3, Styles, "Shape:
corner radius scale" (m3.material.io/styles/shape/corner-radius-scale): extra-small 4 dp, small 8 dp, medium 12 dp (cards),
and full; distinct steps in Material's own roles rather than `field-corner` 4 for radius.md, which would equal radius.sm.
theme2 10:57 (5).

## Role stretches

### `=grouped-label-inset` (the Mac's space.xl)
space.xl on the Mac: **30**, the Mac column of the metrics row `grouped-label-inset` (SwiftUI's grouped `Form` measured on
macOS 27.0.1, LLP 1115.002 §1), a grouped section label's inset, which is not the role space.xl plays. theme2's ruling
(2026-10-19): space.xl is the gap between grouped sections WITHOUT a header on every look. iOS's is 35
(`ios-inset-grouped-section-gap`). The Mac's headerless gap between a `.grouped` Form's sections measures 10 (row to row
46, less the 36-point row), below the Mac's space.lg (20), so it cannot be the step. A gap with a header (56, box to box)
was withdrawn: a header mixes a text height into a spacing. The stretch is the Mac's largest grouped spacing in the table,
in the grouped list's own metrics, and it keeps the Mac's steps in order (6, 8, 10, 20, 30).
