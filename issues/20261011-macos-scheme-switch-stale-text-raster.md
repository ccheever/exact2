# macOS: after a live colour-scheme switch, a rastered text that is not selectable keeps its old colour

**Status:** Open
**Systems:** host/apple macOS, text raster, colour scheme
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-11
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261011-x82-macos-scheme-switch-stale-text-raster.md; LLP 1034 D2 (the host resolves `light-dark()`; the web is the parity oracle); LLP 1044 F4 (why text rasters off the main thread); issues/20261009-macos-small-text-stale-raster.md; main PR #327

## Summary

On the web (the JS target), a live change of `prefers-color-scheme` repaints every text in the other half of its
`light-dark()` colour. On macOS, after a live switch, a text that the host counts as not selectable keeps the old half
when it is drawn from a text raster. It stays that way until the view is drawn again for another reason, such as a
remount. Selectable texts, and texts too small to raster, take the new half.

The Mac host's text-selection rule decides which texts are not selectable (`textSelectable`,
`host/apple/Sources/ExactKit/Mac/TextSelectionMac.swift:382-420`). These are: a text under `user-select: none`; a
text inside a pressable, a `button`, a control or a control role; and a text inside a `header`, `nav` or `footer`, or
a `toolbar`, `tablist`, `menubar`, `menu`, `listbox` or `tree` role. A text rasters when it holds at least
`TextRasterizer.minPixels` (16384 device pixels): about 256 points wide for a 16-point line on a 2x display.

The cause, read from the source (`host/apple/Sources/ExactKit`):

1. An appearance change reaches each node view (`Mac/NodeViewMac.swift:761-781`
   `viewDidChangeEffectiveAppearance` → `reapplyColors`). There, `invalidateText()` (`NodeText.swift:56-71`) drops
   the raster's key and keeps the old surface up: "The old pixels stay up until the new ones replace them."
2. A rastered text paints in `updateLayer` (`Mac/NodeViewMac.swift:961-980`). That calls `textRasters.ensure(self,
   urgent:)` and then `presentTextRaster()`, which shows the surface it already has.
3. `TextRasterizer.ensure` (`Mac/TextRasterMac.swift:66-91`) renders synchronously only for a text's first pixels.
   For a replacement it needs a worker, and there are two (`maxConcurrent`, `:39`). A worker gives its slot back on
   the main queue after it renders (`:88`). So within one display pass, two replacements start and every other text
   is declined (`return false`).
4. Declined texts are asked again by the presenter's pump, `refreshVisibleText` (`Mac/PresenterMac.swift:226-283`),
   through `replaceVisible` and its `needsTextRaster && rastersText` loop. But its candidates are
   `TextViewportIndex(selection.paragraphs)` (`:230`, `:234`), and `TextSelection.paragraphs`
   (`Mac/TextSelectionMac.swift:58-74`) keeps only selectable paragraphs (`:65`, `:152`). So a declined text that is
   not selectable is never asked again, and its old surface stays.

A scheme switch is where this shows, because it changes every text at once. A text below the raster size is painted by
`draw` with the new colours (`textIsSmall`, `Mac/TextRasterMac.swift:210`).

The agent's plain `screenshot` does not show this gap. It captures the view by drawing every text again through
`draw`, with the current colours. `screenshot … window` (the window server's picture) and `sample` (a render of the
view's layers) show what is on screen.

## Why this arose

In T3 PR #417 (task diff-gutter-selection-followups, observation 3), the Files surface was opened in light and the
agent then ran `prefer prefers-color-scheme dark`. Line 1's code kept `#0a0a0a` on the `#111111` surface; its colour
is `light-dark(#0a0a0a, #fafafa)`. The tree label `.agents` kept `#27272a`; its colour is `light-dark(#27272a,
#f5f5f5)`. Both are wide enough to raster, and neither is selectable on the Mac. The code lines sit under a
`column … press=…` (`file-lines`), and the tree's labels are `button role="treeitem"` under `role="tree"`. Meanwhile
`.claude` (by the cause above, one of the two replaced) and the line number 2 (too small to raster) took their dark
halves. With the tab closed and opened again in dark, both were right. The clone has no workaround: a view opened before a live switch can
show unreadable text until it mounts again.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `e2c6fcc4318e1e47e21e5fd4df7a872f01f468ef`. Its
`app.ts` exports an empty `sources`, and `contract build` gives no diagnostic. Every probe text is
`light-dark(#d00000, #00b4ff)`: red in light, blue in dark. "Mount" mounts the probes, so their first pixels are drawn
in the scheme of that moment. S1–S5 are selectable: a plain text, an inherited colour, a component prop, a coloured
run in a paragraph, and a virtualized list's rows. U1–U5 are not selectable on the Mac (three each): a button's label;
`user-select="none"`; a paragraph of runs under a container with `press=` (as the clone's Files surface); a text under
`role="tree"`; and a button's label that inherits the button's `color`. Each U text is at least 420 points wide.
`M1`–`M3` are button labels too small to raster.

```text
// X82: which texts follow a live colour-scheme switch on macOS. Every probe text is
// `light-dark(#d00000, #00b4ff)`: red in light, blue in dark, on a `light-dark(#ffffff, #111111)` page.
// "Mount" mounts the probes, so their first pixels are drawn in the scheme of that moment.
fn inkOf(cls: string): string = cls == "kw" ? "light-dark(#d00000, #00b4ff)" : "light-dark(#d00000, #00b4ff)"

component X82app
  state shown = false
  action toggle
    shown = not shown
  view
    main testId="root" width="100%" height="100%" padding=16 box-sizing="border-box" background-color="light-dark(#ffffff, #111111)" display="flex" flex-direction="row" gap=24
      column width=440 gap=4
        button "Mount" press=toggle testId="mount"
        when shown
          text "Selectable text" font-size=11 color="#888888"
          text "S1 plain MMMMMMMMMMMM" color="light-dark(#d00000, #00b4ff)" font-size=13 font-weight=700 testId="s-plain"
          column color="light-dark(#d00000, #00b4ff)"
            text "S2 inherited MMMMMMMM" font-size=13 font-weight=700 testId="s-inherited"
          Ink(label="S3 component prop MMMM", ink="light-dark(#d00000, #00b4ff)", tid="s-prop")
          text font-size=13 font-weight=700 testId="s-run"
            text "S4 "
            text "run in a paragraph MMMMM" color="light-dark(#d00000, #00b4ff)"
          list virtualized=true testId="s-list" height=72 width=420 estimated-item-height=24
            each n in [1, 2, 3] key=n
              row height=24 align-items="center"
                text `S5 virtualized row ${n} MMMM` color="light-dark(#d00000, #00b4ff)" font-size=13 font-weight=700 testId=`s-row-${n}`
          text "Not selectable on macOS (a control's label, user-select none, a pressable ancestor, a tree)" font-size=11 color="#888888"
          each n in [1, 2, 3] key=n
            button press=toggle width=420 height=22 padding-left=0 border-width=0 background-color="#00000000" display="flex" align-items="center" testId=`u-btn-${n}`
              text `U1 button label ${n} MMMMMMM` flex=1 text-align="left" color="light-dark(#d00000, #00b4ff)" font-size=13 font-weight=700 testId=`u-label-${n}`
          each n in [1, 2, 3] key=n
            text `U2 user-select none ${n} MMMMMM` user-select="none" color="light-dark(#d00000, #00b4ff)" font-size=13 font-weight=700 testId=`u-none-${n}`
          column press=toggle width=420 testId="u-pressable"
            each n in [1, 2, 3] key=n
              CodeLine(n=n)
          column role="tree" width=420
            each n in [1, 2, 3] key=n
              text `U4 in role tree ${n} MMMMMMMMM` color="light-dark(#d00000, #00b4ff)" font-size=13 font-weight=700 testId=`u-tree-${n}`
          each n in [1, 2, 3] key=n
            button press=toggle width=420 height=22 padding-left=0 border-width=0 background-color="#00000000" color="light-dark(#d00000, #00b4ff)" display="flex" align-items="center" testId=`u-ibtn-${n}`
              text `U5 inherited in a button ${n} MMM` flex=1 text-align="left" font-size=13 font-weight=700 testId=`u-inh-${n}`
          text "Not selectable, but too small to raster" font-size=11 color="#888888"
          row gap=8
            each n in [1, 2, 3] key=n
              button press=toggle height=22 padding=0 border-width=0 background-color="#00000000" testId=`u-small-btn-${n}`
                text `M${n}` color="light-dark(#d00000, #00b4ff)" font-size=13 font-weight=700 testId=`u-small-${n}`

component Ink
  props
    label: string
    ink: string
    tid: string
  view
    text label color=ink font-size=13 font-weight=700 testId=tid

component CodeLine
  props
    n: number
  view
    row width="100%"
      text `${n}` width=24 flex-shrink=0 text-align="right" font-family="ui-monospace" font-size=13 line-height="20px" color="light-dark(#565656, #9d9d9d)"
      text flex=1 min-width=0 padding-left=8 font-family="ui-monospace" font-size=13 font-weight=700 line-height="20px" white-space="pre" color="light-dark(#d00000, #00b4ff)" testId=`u-code-${n}`
        each cls in ["kw", "x"] key=cls
          text (cls == "kw" ? "U3 code " : `under a pressable ${n} MM`) color=inkOf(cls)
```

On macOS, the drive is `bun exact.mjs agent macos --size 1000x800 "tap mount" "clock +1000 real" "sample …"
"prefer prefers-color-scheme dark" "clock +1500 real" "sample …" "clock +5000 real" "sample …"`. Each `sample` reads
2 × 160 points across the middle of each text. The second drive mounts in dark, switches to light, then remounts
("tap mount" twice). The Mac's own appearance (Dark) was not changed. The agent pins light at its first operation.
On the web, Chrome mounts in light, then CDP `Emulation.setEmulatedMedia` sets `prefers-color-scheme: dark`, and each
text's computed colour is read with `getComputedStyle`.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| Mounted in light, then `prefer prefers-color-scheme dark` | macOS 26.6.2 (25G83), Apple Silicon, 2x; the agent's `sample` (a layer render) | main `e2c6fcc43` | S1–S5 blue. Of the 15 U texts, 13 stay `#d00000` (the light half) 1.5 s and 6.5 s after the switch: U1–U4 and the first U5. Two (U5 2 and 3) and `M1`–`M3` turn blue | every text `#00b4ff`, as on the web | image (right), record |
| Mounted in dark, then `prefer prefers-color-scheme light`, then a remount | macOS | main `e2c6fcc43` | The same 13 stay `#00b4ff` on the white page; after the remount every text is `#d00000` | every text `#d00000` at the switch | record |
| Drawn at launch in the Mac's Dark, then the agent's own switch to light | macOS | main `e2c6fcc43` | Labels of buttons inside a `scroll` keep `#00b4ff` in the light window (the first app version) | `#d00000` | record |
| Mounted in light, then CDP `prefers-color-scheme: dark`; also loaded fresh in dark | Exact web (JS target) in Chrome 155.0.8059.39 | main `e2c6fcc43` | Every text and run `rgb(0, 180, 255)` after the live switch, and the same fresh in dark | (the reference) | image (left), record |

Each macOS scenario reproduced on its first drive. The same 13 texts stayed in every run: the two drives and the
five launches behind the image's right half.

![X82: after a live switch to dark, Exact web in Chrome and the macOS layers](https://raw.githubusercontent.com/ccheever/exact2/088311b3819b1fd8ece1b7a30c06e4806ae7988b/fw-issues-20261011l/x82-live-switch-web-macos.png)

Record (the contract, the commands, every drive's table, and the code read):
https://raw.githubusercontent.com/ccheever/exact2/088311b3819b1fd8ece1b7a30c06e4806ae7988b/fw-issues-20261011l/x82-record.txt
(sources at the same commit: `x82-app.contract.txt`, `x82-mac-drive.mjs.txt`, `x82-mac-layer-image.mjs.txt`,
`x82-chrome-scheme-probe.mjs.txt`, `x82-compose.py.txt`; also `x82-macos-layers-live-dark.png`,
`x82-macos-agent-screenshot-live-dark.png` (the agent's plain screenshot, every text right), and
`x82-web-js-*.png`).

Not run:
- A real system appearance change. On this Mac the only way is System Settings › Appearance, which is the user's
  setting. Read from the code, it reaches the same `viewDidChangeEffectiveAppearance` path as the agent's
  `NSApp.appearance`.
- `screenshot … window`: the window server gave no picture of the window on this run.
- iOS (`TextRasterIOS.swift` has its own pump), tvOS, Linux, Windows, Firefox and WebKit.
- A colour change of three or more unselectable texts at once without a scheme switch. Read from the code, the same
  decline would leave all but two.

## Constraints

- The web is the parity oracle (`CLAUDE.md`, LLP 1034 D2). On a live `prefers-color-scheme` change, the browser
  resolves every `light-dark()` colour in the JS target's CSS again, at once. A hand-built AppKit app redraws its
  labels in the new appearance as well.
- Raster replacement stays off the main thread (`TextRasterMac.swift`, LLP 1044 F4). The old pixels may stay up until
  the new ones are ready, as `invalidateText` intends. But every text must get its new pixels without a remount,
  whether or not it can be selected.
- Which texts are selectable is AppKit's rule (`textSelectable`, LLP 1115 D8) and stays as it is. The raster pump's
  candidates need not be the selection's paragraphs. They could be every visible paragraph, or a declined `ensure`
  could ask the pump again.
- Texts below the raster size, and selectable texts, keep working as they do.
- `issues/20261009-macos-small-text-stale-raster.md` (#316) is about a paragraph whose geometry crosses below the
  raster size; this issue is about a paint change with the geometry unchanged. No open or closed issue covers a scheme
  switch or a stale raster after a paint change, and neither do the GitHub issues.
- Main PR #327 (head `14493d253`) does not fix it (read from its diff, not run). Its `TextRasterMac.swift` hunks drop a
  raster in `textRasterGeometryChanged` when the node can no longer raster, and clear `layer.contents` in
  `dropTextRaster`. It does not touch `ensure`, `invalidateText`, `viewDidChangeEffectiveAppearance`,
  `refreshVisibleText` or `TextSelection.paragraphs`.

## Acceptance criteria

- In the repro on macOS, read with `sample` or `screenshot … window`, every probe text is `#00b4ff` within a second of
  `prefer prefers-color-scheme dark`, and `#d00000` within a second of switching back, with no remount.
- The same holds with more unselectable rastered texts than raster workers, for instance 40 button labels.
- A text mounted in dark and switched to light is `#d00000` everywhere.
- A macOS test switches the appearance under more than two rastered texts inside a button and under `user-select:
  none`, and checks each raster's colour.
- Selectable texts, small texts, rasters while scrolling (LLP 1044), and #316's case keep working.
