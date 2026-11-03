# JS Canvas 2D loses currentColor for modern CSS colors

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, Canvas 2D, CSS color
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, a Canvas 2D surface whose CSS color is `color(display-p3 ...)`, `oklch(...)`, or another modern color function draws `currentColor` as black. Changing between such colors also fails to trigger the expected style redraw.

`host/web-js/canvas2d.js:63-65` recognizes only comma-separated rgb()/rgba() from getComputedStyle. Browsers retain the other color functions in resolved color serialization, as [CSS Color 4's serialization rules](https://www.w3.org/TR/css-color-4/#serializing-color-function-values) specify. The helper returns null; `ts-draw.js:28` passes that to the recorder, whose `canvas/recorder.js:578` resolves a missing current color to black. The style-change key at `canvas2d.js:68` also collapses every unrecognized color to the same null value.

Run `bun /tmp/exact-review-canvas-current-color.mjs`. Pinned Chromium imports the actual canvas2d.js and uses its registration, geometry and after-commit paths, with a stub drawer recording the color input and a stub replay glue. A real DOM canvas first has computed color `color(display-p3 1 0 0)`, then `oklch(0.6 0.2 30)`. The result is `drawColors=[null]`: no valid first color and no redraw after the change. This verifies the browser serialization and adapter behavior; it does not replay or inspect pixels in a compiled app.

Convert resolved CSS colors through a parser or browser color-conversion path that covers the accepted CSS grammar. Preserve the correct color space through the draw seam and compare the actual resolved color when deciding whether to redraw. Cover currentColor and a subsequent color change for both display-p3 and oklch.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

## Resolution

Resolve CSS colors through the Canvas recorder parser, retaining wide-gamut components and the serialized color in redraw keys. The Rust draw adapter converts modern CSS colors through the browser to its sRGB ABI.

## Verification

`bun test host/web-js/canvas2d-style.test.js` passes currentColor and style redraw checks for display-p3 and oklch. This checks adapter values, not replayed pixels.
