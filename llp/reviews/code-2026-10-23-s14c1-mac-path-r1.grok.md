# Code review: stage 14 C1: macOS draws a template-path icon as its path, round 1 of 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `dc5cbbb17d`.
- **Method:** one brief (sha256 `b46d4655c492a5124effb75bb15b3a9a88ec43f13f1c2700558b8264b261cb1c`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The review is still open. I'll confirm the tint-compositing path in the commit against the surrounding draw code, then write the findings.## MAJOR — Overlapping `;` paths source-over a translucent tint twice

`host/apple/Sources/ExactKit/Mac/IconShapeMac.swift:46` (`paint`) and `:61` (`redraw`)

Each `;` path is its own `CAShapeLayer`, and both `paint` and `redraw` assign the tint `CGColor` — alpha included — as that layer's `fillColor`. Sibling layers composite source-over, so where two paths overlap the ink is `a + a·(1−a)` instead of `a`. A `labelColor` tint (alpha ≈ 0.847) comes out ≈ 0.977 at the junction. The probe glyph is exactly this shape: a ring and a bar that cross.

The draw this replaces does not. `IconPath.image` fills every path as opaque black into one template bitmap, then `contentTintColor` applies the alpha once. Linux does the same on purpose (`host/linux/src/paint/svg.rs:191`): when `d` contains `;` and the colour alpha is below 255, it pushes one opacity, fills each path opaque, then pops. ExactSnapshot masks a single tinted rectangle with the paths (`SnapshotLeaves.swift:293`), so the tint shows through once. Opaque tints (`.textColor`, `alternateSelectedControlTextColor`, an author colour at alpha 255) and single-path icons look the same, which is why the new pixel oracle still passes: it fills opaque black (`IconShapeMacTests.swift:70`, style at `:105`) and `rgb()` drops alpha.

**Fix:** keep each shape's fill at alpha 1 (the tint's RGB only) and apply the tint alpha once on `IconShapeView`'s layer (`opacity`, `allowsGroupOpacity` left on) so the paths composite first and the group fades once. A single path can keep the alpha on its fill. Leave the node's `alphaValue` alone — CSS opacity and the disabled 0.5 already multiply the view, the way `contentTintColor` did.

## Tests

The reference is independent of the code under test. `reference` builds `CGMutablePath`s from the `Seg` cases with Core Graphics and never calls `IconPath`. The host only receives the `%g` `d` string and `"0 0 24 24"`.

The probe does drive a real node through `updateSymbol`. `presenter` puts an image node in a window with `Presenter.apply` (create, frame, children, roots), which reaches `updateSymbol` via `loadImage`. `render` then calls `layoutSymbol` and `Capture.draw` on that page. The leaf-swap, accessibility, and image-size tests use the same presenter.

## What holds

Placement matches a y-down Core Graphics fill at the node's fractional frame, including the flipped `IconShapeView` inside the unflipped clip. `symbolInk` is still the tint, and it is reapplied from `updateSymbol`, `viewDidChangeEffectiveAppearance`, `reapplyColors` / `applyStyle`, `systemColorsChanged`, `retint`, and `rowEmphasisChanged`. Path ↔ symbol keeps one leaf. The leaf is not an accessibility element and not a hit target. `native.symbol` and inspection see `symbolShape`. Implicit `fillColor` / `bounds` / `position` actions are nulled. `contentsScale` follows the window scale and `viewDidChangeBackingProperties` redraws. SF Symbols, bitmaps, control faces, intrinsic size, ExactSnapshot, and iOS are unchanged.

The 2× loop rasterizes into a 2× bitmap of a 1× window, so it checks placement into that bitmap and does not exercise the backing-scale hook. That is a coverage gap, not a wrong assertion.

Verdict: one MAJOR — translucent overlaps paint darker than the template image, the Linux host, and the snapshot; the opaque fractional-origin case holds.
