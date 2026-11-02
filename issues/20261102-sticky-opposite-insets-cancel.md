# Opposite sticky insets cancel and move a tall box above the scrollport

**Status:** Closed
**Severity:** P2
**Systems:** kernel/src/kernel/sticky.rs; host/linux/src/paint.rs
**Author:** Codex
**Date:** 2026-11-02

Reviewed source revision: `5b94c216a6fee8fca42026f5cb2e9a80063da03a`.

`StickyConstraint::offset` computes the end-edge pull and start-edge push from the same natural position, then adds them (`kernel/src/kernel/sticky.rs:44-61`). When a sticky box is taller than its scrollport and both insets are set, the adjustments cancel in part. The box moves above its top inset instead of keeping the start edge pinned. Native paint and hit testing consume this offset directly (`host/linux/src/paint.rs:1303-1328`); the runner's intersection sampler also uses it.

This accepted Contract reproduces the error:

```contract
component App
  view
    scroll width=200 height=100
      column height=500
        box height=20
        box position="sticky" top=0 bottom=0 height=150 width=50 testId="sticky"
        box height=330
```

After layout, the sticky box has natural bounds `[0,20,50,170]`, containing limits `[0,0,200,500]`, and a 100-pixel-high port. At scroll offset 50, the kernel returns vertical displacement 10, placing the box at viewport y=-20. The equivalent HTML/CSS in local Chrome places it at y=0. At scroll offset 20, the kernel also returns y=-20 while Chrome keeps y=0; this is ordinary scroll distance and size, not a numeric boundary.

Executed `/tmp/exact-kernel-sticky-both-probe.rs` against the checkout's warm Rust artifacts and `/tmp/exact-kernel-sticky-both-browser.mjs` against installed Chrome in a fresh headless context. The browser probe blocks network requests and asserts the y=0 result. No platform app was driven; the proof covers the shared placement calculation and its native consumer.

Apply the start-edge constraint to the position after the end-edge adjustment, including the oversized sticky-view rectangle rule, so the start edge wins when both cannot fit. Add a Chrome parity case with both insets and a box taller than the port; the existing corpus covers those conditions separately.

Deduplication: searched current issues and both local `exact2/issues` and `exact/issues`, including closed tickets. `20261102-native-geometry-ignores-sticky-placement.md` concerns frame/point readers omitting this calculation; this issue concerns the calculation itself. Closed `20261009-intersection-visibility-event-design.md` added sticky offsets to intersection reads and does not repair opposite-inset cancellation.

## Resolution

Sticky placement reduces the end inset when needed to contain an oversized box, then applies the start constraint to the adjusted position.

## Verification

The kernel sticky corpus passed, including a new opposing-inset case measured with pinned Chromium 156 at scroll offsets 0, 20, 50, 100 and 400. The oversized box stays pinned until its containing-block limit.
