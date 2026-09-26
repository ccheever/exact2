# LLP 1059: A tablist of symbol-over-label tabs is a tab bar on iOS

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** iOS host (`SegmentsIOS.swift`)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1035.001 D10 (tablists project to a segmented control); LLP 1035.006 07.03 (application tab bars); LLP 1038 (the router's tabs); grnl's FRICTION.md F12, M10

## Summary

A horizontal `role="tablist"` projects on iOS by the shape of its tabs:

| Tabs | iOS presents |
| --- | --- |
| each one image or one label | `UISegmentedControl` (LLP 1035.001 D10, unchanged) |
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
- **D2a — At least the bar's own height.** The bar is sized to
  `sizeThatFits` and sits on the bottom edge of the box, growing upward when
  the box is shorter. An authored row sized for its own tabs is often
  shorter than a tab bar. iOS 26 draws the selected item's title inside the
  selection's glass, which is clipped to the bar's bounds, so a short frame
  turned that title into a line of dots.
- **D4 — The journal says which.** The one-line journal per tablist
  (`tablist #N: …`) now also names `projected to UITabBar`, and the agent
  observation reports `view: UITabBar`. That makes F12's surprise
  discoverable in the logs.
