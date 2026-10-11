---
name: 20261011-x82-macos-scheme-switch-stale-text-raster
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X82: On macOS, after a live colour-scheme switch, a rastered text that is not selectable keeps its old `light-dark()` colour until it mounts again, so the Files surface's code and the file tree's labels stay light in dark

Moved to main `issues/20261011-macos-scheme-switch-stale-text-raster.md` (2026-10-11), where it is tracked. Main PR
[#420](https://github.com/ccheever/exact2/pull/420) filed it (merged `a9741433b`).

## Summary

On macOS, an appearance change drops each text raster's key and leaves the old surface up until a new one replaces it
(`NodeViewMac.swift` `viewDidChangeEffectiveAppearance` → `invalidateText`). `TextRasterizer.ensure`
(`TextRasterMac.swift`) has two workers, so it starts two replacements in a display pass and declines the rest. The
presenter's pump asks the declined ones again (`refreshVisibleText`), but its candidates are the selection's paragraphs
(`TextViewportIndex(selection.paragraphs)`), which hold selectable texts only. A text the Mac counts as not selectable
is never asked again: a button's label, `user-select: none`, a text under a pressable, or a text under a `header`,
`nav`, `footer` or a `toolbar`, `tablist`, `menubar`, `menu`, `listbox` or `tree` role (`textSelectable`). If it is wide
enough to raster (16384 device pixels: about 256 points for a 16-point line at 2x), it keeps the old half of its
`light-dark()` colour until it mounts again. Selectable texts and texts too small to raster follow the switch. On the
web (the JS target), every text follows.

## Why it arose

Task `20261011-diff-gutter-selection-followups` (T3 PR #417, closed), observation 3. The Files surface was opened in
light, then the agent ran `prefer prefers-color-scheme dark`. Line 1's code kept `#0a0a0a` on the `#111111` surface
(`light-dark(#0a0a0a, #fafafa)`, `synInk`'s default), and the tree's `.agents` kept `#27272a` (`light-dark(#27272a,
#f5f5f5)`). Meanwhile `.claude` and the number 2 took their dark halves. With the tab closed and opened again in dark,
both were right. Both texts are unselectable on the Mac and wide enough to raster. The code lines sit under
`column … press=local("surface-files-begin-edit", …)` (`r4-surfaces-files.contract`, `file-lines`). The tree's labels
are `button role="treeitem"` under `role="tree"`. The number 2 is too small to raster. The record did not read the cause
and did not file it.

## Clone workaround

None. A view opened before a live switch can show unreadable text until it mounts again. Examples are the Files
surface's code lines and the file tree's labels; any wide button label, `user-select: none` text, or text under a
pressable or chrome role can do the same. This happens in both directions, light to dark and dark to light. When main
fixes the raster pump, re-drive a live switch with the Files surface open on macOS. Read it with `sample` or
`screenshot … window`, not the agent's plain `screenshot`, which draws every text again. Every text should take its
new half with no clone change.

## Evidence and history

- 2026-10-11, main PR #420 reproduced it on main `e2c6fcc43` in a one-file app. "Mount" mounts the probe texts in one
  scheme, and then the scheme is switched live. Every probe text is `light-dark(#d00000, #00b4ff)`. The selectable
  texts are: a plain text, an inherited colour, a component prop, a run in a paragraph, and a virtualized list's rows.
  The 15 unselectable texts, three of each kind, are: a button's label, `user-select="none"`, code runs under a
  container with `press=`, a text under `role="tree"`, and a button's label that inherits its `color`. There are also
  three button labels too small to raster.
  - macOS 26.6.2, with the agent's `prefer prefers-color-scheme` and the agent's `sample` (a render of the view's
    layers). The Mac's own appearance (Dark) was not changed. Mounted in light and switched to dark, 13 of the 15
    unselectable texts kept `#d00000` 1.5 s and 6.5 s later. Every selectable text, two of the 15 and the small labels
    turned `#00b4ff`. Mounted in dark and switched to light, the same 13 kept `#00b4ff`, and a remount drew them right.
    Each scenario reproduced on its first drive.
  - The Exact web (JS target) in Chrome 155, with a live CDP `Emulation.setEmulatedMedia` switch: every text and run
    turned `rgb(0, 180, 255)`, the same as a page loaded fresh in dark.
  - Not run: a real system appearance change, which would change the user's setting; from the code, it takes the same
    path. Also not run: `screenshot … window`, because the window server gave no picture on this run. The agent's
    plain `screenshot` showed every text right, because it redraws them.

  Open main PR #327 does not change it. Its `TextRasterMac.swift` hunks change `textRasterGeometryChanged` and
  `dropTextRaster`, not `ensure`, the pump or `TextSelection.paragraphs` (read from its diff).
  Image:
  [x82-live-switch-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/088311b3819b1fd8ece1b7a30c06e4806ae7988b/fw-issues-20261011l/x82-live-switch-web-macos.png);
  record: [x82-record.txt](https://raw.githubusercontent.com/ccheever/exact2/088311b3819b1fd8ece1b7a30c06e4806ae7988b/fw-issues-20261011l/x82-record.txt).
