# Code review: stage 14 C1: macOS draws a template-path icon as its path, round 2 of 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `878a3bfa2a`.
- **Method:** one brief (sha256 `4d742c2eb0257586141677cf1457afa93a0c8786bde198a09880d95f80d64135`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

I'll re-read the round-2 icon drawing and the tests so the checklist and any findings cite the current lines.Both round-1 fixes are in place. The translucent tint is applied once, paths are clipped to the placed em square, and the rest of the branch does not add a defect.

## Translucent tint

`paint` fills every shape with the tint’s RGB at alpha 1 and sets the leaf’s `alphaValue` to the tint’s alpha (`IconShapeMac.swift:62-69`). Those shapes are the only children of `group`, whose opacity stays 1, and the view layer’s `allowsGroupOpacity` is set. Overlaps are resolved inside the group, then the leaf fades that result once. That matches the template image, Linux’s single opacity push, and Snapshot’s one masked rect.

## Em-square clip

`group.masksToBounds` is true and `redraw` sets `group.frame` to `IconPath.placed`’s square (`IconShapeMac.swift:27-31`, `76-80`). Each path is mapped by the viewBox fit, then translated by `-square.origin` so it is in the group’s local space (line 80).

## Opacity on every re-tint

`paint` is the only writer of the leaf alpha, and each of these reaches it:

- Appearance calls `tintIconShape()` before the scheme-color return (`NodeViewMac.swift:843`), so an unsaid label color updates too. A scheme color also goes through `reapplyColors` → `applyStyle` → `updateSymbol`.
- Row emphasis calls `tintIconShape(emphasized)` (`TableInkMac.swift:74`), which paints `alternateSelectedControlTextColor` or `symbolInk`, alpha included. Leaving the row paints the ordinary ink again.
- `retint` treats `hasSymbolLeaf` as a reason to `reapplyColors` (`NodeColorMac.swift:67`), and that pass reads `inEmphasizedRow` again.
- Style updates always call `updateSymbol`, and `tintIconShape()` sits outside the `symbolKey` cache (`NodeSymbolMac.swift:60`).
- `systemColorsChanged` uses the same `hasSymbolLeaf` guard (`NodeViewMac.swift:850-852`).
- `symbolColorWritten` and a new image source also call `updateSymbol`.

## CSS opacity, disabled alpha, visibility

Those stay off the leaf. The opacity op writes the node’s `alphaValue` from `presentedAlpha`, halved when the control is disabled (`PresenterMac.swift:1205`, `PressLookMac.swift:113`). Press and look fades animate that same node layer. `visibility: hidden` sets `isHidden` on the clip and the shape (`CssVisibility.swift:54-55`) and does not write `alphaValue`. The leaf’s tint alpha multiplies with the node’s opacity, which is where a template image’s tint alpha used to sit.

## Group frame, object-fit, padding, corners, layout

`layoutSymbol` puts the clip on `contentBox()` and the shape on the clip’s bounds, then places the icon (`NodeSymbolMac.swift:63-67`). That box is the kernel content box, or the bounds inset by border and padding for a native field. `IconPath.placed` centers the em square: fill and contain use the min side, cover the max side, none the em size, and scale-down the smaller of those. The rounded-corner mask is the border-box path on the clip, shifted by `-content.origin` (`NodeSymbolMac.swift:87-90`), so corners clip the already faded leaf and do not shrink the em square. A cover square may extend past the shape view; the clip’s `masksToBounds` and that mask keep it in the content box, as the enlarged image view did. `NodeView.layout` calls `layoutSymbol` on every layout (`NodeViewMac.swift:1314`), and `updateSymbol` calls it on style and prop updates. A frame op updates the view frame, and the icon follows on that same layout pass, together with an SF Symbol’s image view.

## Implicit animations

`redraw` only writes properties whose actions are `NSNull`: the group’s bounds, position, `contentsScale`, and sublayers, and each shape’s path, `fillColor`, bounds, position, and `contentsScale` (`IconShapeMac.swift:30`, `46`, `76-84`). `alphaValue` is set inside `CATransaction.setDisableActions(true)` (lines 65-67). The fill is assigned again after that transaction, and the shape’s `fillColor` action is still null. Adding the group does not fade in.

## Backing-scale test

It does exercise `redraw`. `Scaled` overrides `backingScaleFactor`, which is what line 77 reads. The create op draws before the node is in any window, so that first pass records the screen scale. Joining the window sends `viewDidChangeBackingProperties`, and the override calls `redraw` against the Scaled window, which is why the assert sees 3. The change to 1 is that same override called by hand, because changing the getter does not make AppKit deliver the callback; the test says so (`IconShapeMacTests.swift:442-458`). `contentsScale` defaults to 1, so a measured 3 means `redraw` ran. The test checks that property, not pixels.

## What the new tests actually check

The overlap test renders two overlapping rects for `rgba(0,0,0,.5)` and for `labelColor`. It asserts the leaf alpha and `layer.opacity` match and are below 1, each fill’s alpha is 1, and the overlap’s alpha equals each single-path sample and an independent Core Graphics fill of the union at that alpha (within 1). The sample points are not symmetric, so an upside-down icon would miss them. It covers the initial paint. Re-tint of alpha is in the production calls above; the appearance test checks fill RGB.

The clip test uses a 48×24 `contain` box, font-size 24, and a bar from x=-12 to 36. Empty gutters and a solid square, including x=12 and x=35, lock the group’s position and `masksToBounds`. The bar covers the whole square, so those pixels would still pass if the `-square.origin` translation were dropped while the group stayed put. The square-origin-0 probes do not exercise that translation either. The translation itself is the right concatenation (line 80).

## Rest of HEAD~2..HEAD

SF Symbols and bitmaps stay on `NSImageView`. A path, then a symbol, then a path keeps one leaf in the same clip. Accessibility stays on the node, and the leaf is not an element. Intrinsic size and control faces still read the em-square template image. `hasSymbolLeaf` keeps path icons out of the bitmap draw, the image plan, and capture-pixels. iOS and Snapshot are unchanged; the `IconPathTests` and `CssVisibility` edits are in the macOS branches. The suite was not run in this review.

No BLOCKER, MAJOR, MINOR, or NIT.

Both round-1 fixes are correct, and the branch introduces nothing new.
