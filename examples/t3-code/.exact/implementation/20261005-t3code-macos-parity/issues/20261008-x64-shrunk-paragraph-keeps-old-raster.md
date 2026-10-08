---
name: 20261008-x64-shrunk-paragraph-keeps-old-raster
plan: 20261005-t3code-macos-parity
status: filed
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/316
reproduced_on: main 475043d20 (after #305, 4fe878a13) and the feature branch's framework (c0475fbaa), one-file app
---

# X64: a paragraph that shrinks below the text-raster size keeps painting its old pixels (macOS)

## Summary

On the macOS host, a wrapped paragraph that the host draws from a text raster keeps showing that
raster after an update makes the paragraph small enough to be drawn directly (`textIsSmall`: fewer
than `TextRasterizer.minPixels`, 16,384 device pixels). The view tree, `layout` and the agent's
default `screenshot` (the capture path) all have the new text; the window's own pixels show the old
lines at their old geometry, clipped by a `line-clamp` box or overflowing an unclamped one. Toggling
back to a large paragraph repaints correctly, and a paragraph that stays above the threshold (a wide
one-line box) updates correctly. Expected (CSS): the new text from the first frame after the update.

Not the same as [#300](https://github.com/ccheever/exact2/issues/300) (a `line-clamp` text mounted
after launch paints its last line without "…", fixed by main #305): that one has the right string, and
this one still reproduces on main after #305. Not X62 (window-level popups clipped by ancestors) or X59
(the first layout of a clamped text that replaces a wrapped one). It shares `TextRasterMac.swift` (the
text raster's first-pixels and replacement paths) with [#291](https://github.com/ccheever/exact2/issues/291)
and #300/#305; a fix there should be checked against all three.

## Why this issue arose

### The T3 Code behavior
`ProviderInstanceCard` (`apps/web/src/components/settings/ProviderInstanceCard.tsx:907`) draws a
provider list row's status as `line-clamp-2` text. After Reconnect the row reads the provider's new
status ("Authenticated · ChatGPT") at once, as the editor does.

### What exact2 does today
One-file app (`bun scripts/exact.mjs new <dir>`, an `app.ts` with the template's `greeting` source):

```contract
component X64app
  resource greeting = greeting("X64app") as shape Greeting
  state short = false
  action toggle
    short = not short
  derive status = short ? "Disabled" : "Not authenticated · Sign in with ChatGPT to use Codex."
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="light-dark(#ffffff, #111111)" color="light-dark(#111111, #eeeeee)" font-size=14 line-height="20px"
      button press=toggle testId="t" width=120 padding=6 border-width=1 border-style="solid" border-color="#cccccc"
        text "toggle"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status line-clamp=2 testId="clamped-2"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status line-clamp=1 testId="clamped-1"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status testId="plain"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status white-space="nowrap" overflow="hidden" text-overflow="ellipsis" testId="nowrap"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        each s in [status] key=s
          text s line-clamp=2 testId="keyed"
      column width=420 border-width=1 border-style="solid" border-color="#cccccc"
        text (short ? "Disabled" : "Not authenticated · Sign in with ChatGPT to use Codex. Not authenticated · Sign in with ChatGPT.") testId="wide"
```

`bun exact.mjs mac`, then `bun exact.mjs agent macos --size 480x420 "screenshot before.png window" "tap t" "clock settle" "screenshot after.png window" "screenshot after-capture.png" tree "layout clamped-2" "tap t" "clock settle" "screenshot back.png window"`:

| Row | Box before → after (pt) | Device pixels after | Window pixels after the tap | Tree / capture |
| --- | --- | --- | --- | --- |
| `clamped-2` | 200×40 → 200×20 | 16,000 (small) | "Not authenticated · Sign in" (old first line, clipped) | "Disabled" |
| `clamped-1` | 200×20 → 200×20 | 16,000 (small both times) | "Disabled" | "Disabled" |
| `plain` | 200×40 → 200×20 | 16,000 (small) | both old lines, the second overflowing onto the next box | "Disabled" |
| `nowrap` | 200×20 → 200×20 | 16,000 | "Disabled" | "Disabled" |
| `keyed` (`each … key=s`) | a new node | 16,000 | "Disabled" | "Disabled" |
| `wide` | 420×40 → 420×20 | 33,600 (rastered) | "Disabled" | "Disabled" |

`layout clamped-2` after the tap: `space viewport 25,71 200×20`. The second tap (back to the long
text) repaints every row correctly. Evidence: [06-x64-one-file-app](https://raw.githubusercontent.com/ccheever/exact2/0574dbb319f94727f96392c60d55e220e77a646f/fix-provider-auth-state/06-x64-one-file-app.png)
(the feature branch's framework, c0475fbaa) and [06b-x64-on-main](https://raw.githubusercontent.com/ccheever/exact2/d385ab70b347b1bed6e04a67227b7f2514c2d292/fix-provider-auth-state/06b-x64-on-main.png)
(main `475043d20`, which contains #305's `4fe878a13`: the same rows stay stale); the app's
[view](https://raw.githubusercontent.com/ccheever/exact2/8c7f04f8bb6fac0d4264ceb0e46a3d9d3eac542f/fix-provider-auth-state/x64-one-file-app.contract.txt).

Reading of the source (not instrumented): `canRasterText` refuses a paragraph whose box is
`textIsSmall` (`host/apple/Sources/ExactKit/Mac/TextRasterMac.swift:189-214`), so the view goes
from `updateLayer` (raster) to `draw(_:)`. `textRasterGeometryChanged()` (same file) keeps the
accepted surface up "at its original dimensions" until new pixels replace it, and moves it into
`textRasterOverflowLayer` when the frame no longer matches; nothing replaces it, because the
rasterizer only serves paragraphs that `rastersText`. Re-run on main `475043d20` (2026-10-08, after #305): reproduces as above.

### Where the clone hits it
- `providers.contract` ProvidersPanel: the list row's `line-clamp=2` status. After Reconnect the
  row went from "Not authenticated · Sign in with ChatGPT to use Codex." (two lines) to
  "Authenticated · ChatGPT" (one line, small) and kept painting "Not authenticated · Sign…" until
  Providers was reopened (#298 bug 19). Switching Codex off reproduces it on any lane ("Not auth"
  under a "Disabled" tree).
- `providers.contract` ProviderEditor: the editor's status detail kept its words but moved up beside
  a shorter lead after Disconnect, and its old two-line raster ran under the Display name field.

Workaround (fix-provider-auth-state): both texts are keyed by their status (`each status in
[row.status] key=status`; the editor's line by its parts), so a new status is a new node. Other
clone texts whose strings change in place can still show it; none is known to shrink below the
threshold in a reported flow.

## Why it must be resolved
Text that the tree says changed keeps showing its old words, so a status reads as the opposite of
what it is ("Not authenticated" for a signed-in provider) until something remounts it. Every clamped
or wrapped label whose content can shrink is exposed; a key per value is a workaround, not a fix.

## Filed upstream (2026-10-08)
Filed as [#316](https://github.com/ccheever/exact2/issues/316), reproduced on main `b896050d7`. The T3 rows it blocks wait for the main fix of #316; the clone keeps its workaround (the provider status texts are keyed by status) until then.

- **Fixed by [#327](https://github.com/ccheever/exact2/pull/327)** (open on main, 2026-10-08); this resumes in the main-adoption round after it merges. Then #312's status-keyed redraw goes (the provider list row's status `each status in [row.status] key=status`, and the editor's status line keyed by its parts).
