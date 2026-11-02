# Native frame and point reads ignore sticky positioning

**Status:** Closed
**Severity:** P2
**Systems:** runner/src/geometry.rs, runner/src/geometry/intersection.rs
**Author:** Codex
**Date:** 2026-11-02
**Commit:** 5b94c216a6fee8fca42026f5cb2e9a80063da03a

After scrolling, `frame(id)` reports a sticky element's ordinary scrolled position, and `elementFromPoint` cannot find it where it is pinned. Native actions that position overlays or identify a drop target from these reads get the wrong element or coordinates. Descendants inherit the same missing offset.

`runner/src/geometry.rs:529-545` composes only `own_transform` and ancestor scroll offsets for a frame. The point walk likewise uses `own_transform` at lines 1283-1293. Neither path applies `Kernel::sticky_constraint`. The intersection sampler already applies that constraint at `runner/src/geometry/intersection.rs:192-226`, so these reads disagree with the existing sticky placement calculation.

Reproduced on this Mac with a valid Contract compiled at this commit:

```text
component App
  view
    scroll id="scroller" width=200 height=100
      column height=500
        box id="sticky" position="sticky" top=0 width=50 height=40
```

Boot with `Kernel::with_monospace()`, lay out the root at 200×100, and report an 80 px vertical offset for `scroller` with `Scrolled::note`. Read through `GeometryEnv::new(kernel, &scrolled, &geometry::KERNEL, &[])`, the native geometry implementation used by action evaluation:

- The kernel's sticky constraint returns `(0, 80)` at that scroll offset, keeping the box at viewport y=0.
- `Stdlib::Frame` for `sticky` returns `(x=0, y=-80, width=50, height=40, provisional=false, unavailable=false)`.
- `Stdlib::ElementFromPoint(10, 10)` returns `some("scroller")`, rather than `some("sticky")`.

The executed standalone probe `/tmp/exact-runner-sticky-geometry-probe.rs` asserts all three results against the checkout's existing debug artifacts. No platform app, network or runtime process was started; the evidence covers the shared native geometry reader, not a live presenter. The web's browser geometry reader is outside this finding.

Share the sampler's sticky placement calculation with frame and point reads, including its conversion from reported scroll coordinates. Cover a pinned box, its descendant and the point where its unpinned position would have been.

Deduplication: searched this checkout's issues and both local `exact2/issues` and `exact/issues`, including closed tickets. Closed `20261009-intersection-visibility-event-design.md` records the earlier sticky intersection correction; that fix is present here, while the frame and point paths still omit it. The current authored-border hit-testing ticket concerns a separate clip inset.

## Resolution

Native frame and point reads share the intersection sampler's sticky placement transform. Descendants inherit the same placement.

## Verification

`sticky_frames_and_point_reads_follow_the_pinned_box` passed for the pinned box and its child, their frame positions, hits at the pinned position, and absence of either target at its old position. The runner geometry suite passed.
