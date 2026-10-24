# C2 Apple (x2/c2-apple, Swift + Rust + round 1 fixes), round 2, Grok (xhigh)

The review was mid-verdict when the last session ended. I'll re-check the draw sites, the round-1 redraw and key fixes, and the pixel tests against the code before answering.The draw sites are concentrated. I'll read the fill path, the in-place rule update, and the new tests next.**RIGHT.** No findings.

The even-odd rule is honored on every Apple draw of an icon path, a nonzero icon stays byte-identical, and a rule set or cleared on the same node redraws. The new pixel tests sample the hole and fail if the rule is ignored.

1. **Draw sites.** Page images go through `IconPath.rule` / `iconPathImage` (`ExactKit/IconPath.swift:14`) on iOS and `IconShapeView.show` (`IconShapeMac.swift:50`) on macOS. Widgets and Live Activities share `SnapshotLeaves.swift:298-303` (`FillStyle(eoFill:)`). Toolbars, menus, tab bars, segments, grouped rows, badges, the nav bar, buttons, shortcuts, and the sidebar all draw through `FaceSymbol.image` / `pathImage`, which strips `evenodd|` and fills with that rule (`FaceSymbol.swift:55-60`). Each `;` path is filled on its own. An `sf` name still wins and sends no path. `GroupedListIOS.swift:916` is a list-skirt mask, not an icon.

2. **Nonzero icons.** `push_json` and `props_for` insert `symbolFillRule` only when `even_odd` is set (`icon_path.rs:65-67`, `paragraph.rs:622-624`). The pinned nonzero fragment is the old `symbolPath` + `symbolViewBox` string. Face tokens stay `icon-path:` + viewBox + `|` + path for `nil` and `"nonzero"` (`FaceSymbol.swift:29`). The symbol cache key appends `:` + rule only when the prop is present (`NodeSymbolMac.swift:22`, `NodeViewIOS.swift:538`), so a nonzero key is the previous string.

3. **Rule set or cleared in place.** Both `applyProps` write props, then call `updateSymbol` when `symbolFillRule` is set or cleared (`NodeViewMac.swift:986-991`, `NodeViewIOS.swift:1109-1122`). The key includes the rule, so macOS `show` resets `fillRule` and iOS rebuilds the bitmap. On iOS, `showSymbol` installs the new em image before `layoutSymbol`'s size check (`NodeViewIOS.swift:543-582`), and a box-sized redraw reads the current rule. Toolbar items store the full token and replace the image when it changes (`ToolbarMac.swift:623-636`); props ops are in `WindowToolbarHost.read`. Segment keys are `"\(points)|\(symbol)"` (`SegmentsIOS.swift:446`). Grouped `Row` equality includes that token.

4. **`icon_name`.** It matches `Icon::of_row(...) == Icon::parse` (`icon_path.rs:82-87`), so the marker is stripped and `even_odd` is part of equality. A nonzero row still matches; an even-odd row is distinct from the same `d` nonzero. No other `icons.path` or `icon|` split remains under `host/apple/src`. The reader scan waives a line within four lines of `Icon::of_row(`.

5. **Wire bound.** `content_region.rs:398` adds `("symbolFillRule".len() + "evenodd".len() + 6) * 6` = 162. The fragment `,"symbolFillRule":"evenodd"` is 27 bytes, and `projection_size` is an upper bound checked against `packet_wire_bound`. The prop is synthesized, so the props loop does not count it again.

6. **Tests.** They fail if the rule is ignored:
- `IconPathTests.inked` and `drawn` (`IconPathTests.swift:154-186`, `218-221`) sample row 4 (band) and row 12 (hole). At scale 2 the sample pixels for `(40.5, 60.25)` and `(100.33, 60.67)` sit inside the band and the hole. A filled hole returns `[true, true]` against `[true, false]`.
- `IconShapeMacTests` (`IconShapeMacTests.swift:214-280`) covers six origins, whole and fractional, at 1x and 2x, against an even-odd reference. The nonzero mutation expects `"outside"`.
- `SnapshotDriverTests` (`SnapshotDriverTests.swift:626-643`) samples band `(23,7,4,4)` and hole `(23,23,4,4)` at `(0,0)`, `(0.5, 0.25)`, and `(0.67, 0.69)`. Those rects stay inside the scaled frame. Ignoring the rule inks the hole.
- The toolbar test (`FaceSymbolTests.swift:127-138`) samples only the center (the hole), not the band. Ignoring the rule yields alpha 1 against 0; a blank image fails the cleared-rule assert. The shared `pathImage` is sampled at rows 4 and 12 in `testAnEvenOddFacesTokenCarriesItsRule`.

Round 1 holds: the props trigger is on both platforms, the nonzero symbol key is unchanged, and the pixel tests above catch a winding fill. The toolbar check is hole-only; iOS `drawn()` is fractional-origin, with the integer raster covered by `inked()`.
