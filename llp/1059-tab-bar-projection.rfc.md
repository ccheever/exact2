# LLP 1059: A tablist of symbol-over-label tabs is a tab bar on iOS

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** iOS host (`SegmentsIOS.swift`); macOS host (`SegmentsMac.swift`, D2a's segmented control only)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1035.001 D10 (tablists project to a segmented control); LLP 1035.006 07.03 (application tab bars); LLP 1038 (the router's tabs); grnl's FRICTION.md F12, M10


**Ruled (Charlie, 2026-09-27, after the review of PR #47):** projecting to a native tab bar stays automatic, as the full platform. But the bar's own height (`sizeThatFits`) is reported back to layout, so the kernel reserves the room instead of letting the bar draw outside its box. D2a's overflow is removed: the host reports the measured size through the intrinsic seam and the bar fills the kernel box. LLP 1035.001's projection table lists the three shapes (segmented control, tab bar, authored views) together.

## Summary

A horizontal `role="tablist"` projects on iOS by the shape of its tabs:

| Tabs | iOS presents |
| --- | --- |
| each one image or one label | `UISegmentedControl` (LLP 1035.001 D10); its native height is returned to layout as in D2a |
| each exactly an `image "symbol:…"` plus a text label | `UITabBar` in the tablist's own box |
| anything else | kept as authored |

A tab bar item has exactly the second shape. A segment cannot show it.

## Design

- **D1 — Shape decides, not a new prop.** The web's tablist is the
  standard, and ARIA has no separate role for a tab bar.
  - An icon over a label is how iOS draws a tab bar item, and a segmented
    control never shows both.
  - An app that wants a tab bar writes the tab bar it would write for the
    web, with symbols.
- **D2 — Layout stays authored.** The bar fills the tablist's whole box,
  and the app places that box (at the bottom, over the safe area). The bar
  draws its own background and keeps its items clear of the home indicator.
  The authored tabs are hidden, as for segments. Selection is Contract's:
  - a tap presses the authored tab;
  - `aria-selected` selects the item.
- **D3 — The app's accent, the platform's face.** The selected item takes
  the selected symbol's authored `tint-color`. Everything else is the bar's
  own look.
- **D2a — The measured height belongs to layout.** The host reports
  `sizeThatFits` through the existing intrinsic-size seam, after the creating
  batch. Its height supplies the tablist's automatic minimum height, without
  changing authored rows or introducing an aspect ratio. An explicit CSS
  `min-height` remains authoritative. The bar's frame is exactly the kernel
  box; it never grows upward outside it. Resizing remeasures; ending the
  projection clears the measurement and restores authored layout.
  A segmented control reports its `intrinsicContentSize` through the same
  seam and cache (2026-10-05, PR #93) and fills the tablist's content box,
  so a text-only row no longer squashes it to the height of its labels. Its
  height does not follow the box's width (nor Dynamic Type or bold text,
  measured on the iOS 27.1 simulator), so it is measured at any width and a
  resize does not remeasure. Under `box-sizing: border-box` the report adds
  the tablist's vertical padding and border, which is why `box_sizing`
  crosses to the Apple presenter. macOS's `NSSegmentedControl` does the
  same (2026-10-05): an unsized row of text tabs had clipped it to 18 pt in
  Caltrain's window, and the tablist now takes the control's 24.
- **D4 — The journal says which.** The one-line journal per tablist
  (`tablist #N: …`) now also names `projected to UITabBar`, and the agent
  observation reports `view: UITabBar`. That makes F12's surprise
  discoverable in the logs.
