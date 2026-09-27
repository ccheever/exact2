# A viewport resize during a layout move: the web snaps, macOS keeps animating, Linux snaps a step late

**Status:** Open
**Systems:** Apple host (`host/apple/src/presence.rs`, `host/apple/src/host.rs` resize, `Sources/ExactKit` window resize path), Linux host (`host/linux/src/presence.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063 D6 (a resize takes new boxes with no animation, on every host), `issues/20260927-presence-cross-host-timeline.md` (the recorder that found it)

`bun host/web/parity.mjs --presence` records one 17-step timeline on each host. When the viewport is resized while a `layout-transition` move is running:
- **Web:** snaps at once (the 2026-09-27 fix). This is D6.
- **macOS:** keeps animating. `resize_inner` sets `presence.snap` (`host/apple/src/host.rs`), but the recorded boxes keep moving, so the window-resize path either doesn't reach it or the snap doesn't retire a move already in flight.
- **Linux:** snaps on the next clock step, not in the resize's own frame.

Linux also drops exit ghosts at once, but that is declared (LLP 1063 D8) and not in scope here.

**Fix:** on Apple and Linux, a resize retires every running `Property::Layout` animation and presents the new boxes in the same frame. The recorder's resize steps should then match on all three hosts. Sweep iOS when a simulator is available.
